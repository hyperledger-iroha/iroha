//! Production service boundary for the single Sumeragi v2 reducer owner.
//!
//! The reducer itself remains serialized on the Sumeragi thread. Potentially
//! blocking signing, body fsync/validation, state application, and certified
//! body serving execute on one ordered I/O worker and return tagged
//! completions. Control and recovery remain committee-wide, while first-send
//! body chunks are limited to Set A.
use super::v2_core::{
    CanonicalIdentityProjection, Committee, EventTag, IDENTITY_DOMAIN_PAYLOAD,
    IDENTITY_DOMAIN_PEER, IDENTITY_DOMAIN_PROCESS_LOCAL, IDENTITY_KIND_MERGE_ENTRY,
    IDENTITY_KIND_NETWORK_RESPONSE, IDENTITY_KIND_PEER, IDENTITY_KIND_REFERENCE_DIGEST,
    IDENTITY_KIND_REPLY_DELIVERY_ROUTE, IDENTITY_KIND_REPLY_PAYLOAD,
    IDENTITY_KIND_REPLY_SOURCE_KEY, IDENTITY_KIND_REPLY_WRITER_OCCURRENCE,
    IDENTITY_KIND_SIDECAR_CHUNK, IDENTITY_KIND_SIDECAR_PAYLOAD, IDENTITY_KIND_SIDECAR_REQUEST,
    IDENTITY_KIND_SIDECAR_RESPONSE, ProductionReliableFlushTraceProjection,
    check_production_reliable_flush_worker_transition,
};
#[cfg(test)]
use super::v2_core::{
    Generation, production_reliable_flush_trace_refines_outbound_ownership_kernel,
};
#[cfg(test)]
use super::v2_runtime::RuntimeQueueSnapshot;
use super::{
    FairV2Ingress, FairV2IngressOwnershipEvidence, InboundBlockMessage,
    message::{
        BlockMessage, BlockMessageWire, KuraReplicaAdvertV1, LaneHistoricalRecoveryPayloadV1,
        LaneHistoricalRecoveryRequestV1, LaneHistoricalRecoveryResponseV1,
    },
    output_guard::{ConsensusFailStopOperation, ConsensusOutputGuard, ConsensusOutputPermit},
    v2_apply::{
        RecoveredDecisionApplyTaskV1, RecoveredDecisionApplyWorkerResultV1, V2ApplyService,
    },
    v2_body_store::{
        BodyStoreCompletion, BodyValidationCompletion, DurableBodyReceipt,
        DurableCertifiedServeBodyReadbackV1, V2BodyRetirementJob, V2BodyStore,
        V2BodyStoreInstanceIdentity, ValidatedBodyReceipt,
    },
    v2_certified_serve_payload_store::{
        AuthenticatedCertifiedServePayloadRecoveryCut, CertifiedServePayloadStoreInstanceIdentity,
        CertifiedServePayloadStoreV1, CertifiedServeRetirementAuthenticationErrorV1,
    },
    v2_chunks::{EncodedV2Payload, V2ChunkError, V2ChunkSession, encode_payload},
    v2_effects::{
        ApplyTask, AuthenticatedChunkDisposition, BodyFetchTask, BodyStoreTask, BodyValidationTask,
        CertifiedBodyFetchCompletionDisposition, CompletionDisposition,
        ConsensusBroadcastDisposition, ConsensusSignTask, DurableApplyCompletion,
        EffectExecutorError, EffectExecutorStatus, EffectRuntime, EffectTransportError,
        EffectWorkId, PayloadChunkLifecycleDisposition, PendingTipRecoveryAttemptResult,
        PostFinalityCleanupOutcome, PostFinalityCleanupTarget, V2EffectExecutor, V2EffectServices,
    },
    v2_lane_work::{
        DurableLaneRolloverAuthority, V2LaneWorkAdapter, V2LaneWorkEffect,
        durable_historical_lane_output_source_hash, lane_output_identity,
    },
    v2_lifecycle_coordinator::{
        AuthenticatedSchedulerInputsFactory, CertifiedFetchBodyPersistenceCompletion,
        CertifiedFetchBodyPersistenceId, CertifiedFetchBodyPersistenceTask,
        CertifiedServeTerminalReplayAuthorizationV1, ClaimedCertifiedServeDispatchV1,
        LifecycleIngressIoTargetKind, LifecycleIngressIoTargetSeal,
        PreparedLifecycleIngressSelector, PreparedRecoveredDecisionApplyDispatch,
        PreparedRecoveredLifecycleSignDispatch,
        ProductionLifecycleServeRetirementAuthenticationPermitV1,
        ProductionV2CompletionObserverActivationPermitV1, RecoveredDecisionApplyDispatchKeyV1,
        RecoveredDecisionFetchBodyPersistenceCompletionV1,
        RecoveredDecisionFetchBodyPersistenceTaskV1, RecoveredDecisionFetchDispatchKeyV1,
        RecoveredLifecycleSignDispatchIdentityV1, RecoveredLifecycleSignDispatchKeyV1, TurnLease,
    },
    v2_runtime::{
        LeaderWireRuntimeTerminal, RuntimeLifecycleOrdinalSource, RuntimeQueueLaneSnapshot,
        SerializedV2Runtime,
    },
    v2_transport::{
        AuthenticatedCertifiedBodyRequest, AuthenticatedPayloadChunk, V2TransportError,
        authenticate_certified_body_request_with_validator_pops,
    },
};
use crate::{
    EventsSender, IrohaNetwork, NetworkMessage,
    kura::{Kura, KuraReplicaAdvertSourceV1, KuraV2CommitReceipt},
    lane_consensus::LaneDrainVoteV1,
    merge_sidecar::{
        CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarChunkAdmission,
        CertifiedMergeSidecarChunkV1, CertifiedMergeSidecarCloseAckV1,
        CertifiedMergeSidecarClosedPrefix, CertifiedMergeSidecarMessage,
        CertifiedMergeSidecarRequestV1, CertifiedMergeSidecarSemanticSequenceV1,
        CertifiedMergeSidecarStreamEpochV1, MergeSidecarError, reliable_flush_topic_tag,
    },
    native_amx::NativeAmxMessage,
};
use iroha_config::parameters::{
    actual::{
        KURA_REPLICA_ADVERT_REFRESH_INTERVAL_MIN,
        sumeragi_v2_exact_output_shared_ownership_capacity,
        validate_sumeragi_v2_exact_output_geometry,
    },
    defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT,
};
use iroha_crypto::{Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{
    NetworkId,
    block::{
        BlockHeader, CertifiedMergeLedgerReference,
        consensus::{
            LaneBlockCertificateV1, LaneBlockProposalPayloadHintV1, NativeAmxAttestationBodyV2,
            NativeAmxPhase,
        },
        consensus_v2 as wire, decode_framed_signed_block,
    },
    merge::MergeCommitteeSignature,
    nexus::LaneId,
    peer::PeerId,
};
#[cfg(test)]
use iroha_p2p::network::{
    NetworkActorAdmissionTicketTestFixture, NetworkReplyFlushAckTestFixture,
    NetworkReplyRouteTestFixture,
};
use iroha_p2p::{
    Post, Priority,
    network::{
        NetworkActorAdmissionError, NetworkActorAdmissionRejection, NetworkActorAdmissionTicket,
        NetworkReplyFlushAck, NetworkReplyFlushAckStatus, NetworkReplyRoute,
        NetworkReplyRouteError, NetworkReplyRouteSourceUpdate, NetworkReplyRoutes,
        NetworkReplyRoutesObservedMergeReceipt, NetworkReplyRoutesStrictMergeReceipt,
        NetworkReplySourceKey, ReliableProgressClass,
        message::{ClassifyTopic as _, ProgressReconstruction},
        reliable_progress_class,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering as AtomicOrdering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
fn reliable_flush_typed_identity<T>(
    domain: u8,
    kind: u8,
    hash: HashOf<T>,
) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(domain, kind, *hash.as_ref())
}
fn reliable_flush_hash_identity(domain: u8, kind: u8, hash: Hash) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(domain, kind, *hash.as_ref())
}
fn reliable_flush_peer_identity(peer: &PeerId) -> CanonicalIdentityProjection {
    reliable_flush_typed_identity(IDENTITY_DOMAIN_PEER, IDENTITY_KIND_PEER, HashOf::new(peer))
}
fn reliable_flush_ordinal_halves(ordinal: u128) -> (u64, u64) {
    let high = u64::try_from(ordinal >> u64::BITS)
        .expect("high half of a u128 actor ordinal is representable as u64");
    let low = u64::try_from(ordinal & u128::from(u64::MAX))
        .expect("low half of a u128 actor ordinal is representable as u64");
    (high, low)
}
fn reliable_flush_usize(value: usize) -> Result<u64, MergeSidecarError> {
    u64::try_from(value).map_err(|_| {
        MergeSidecarError::FlushIdentityMismatch(
            "sidecar flush identity field is not representable as u64",
        )
    })
}
pub(crate) fn reliable_flush_trace_projection(
    admission: &CertifiedMergeSidecarChunkAdmission,
    status: NetworkReplyFlushAckStatus,
    flushing_before: u64,
    flushing_after: u64,
    admitted_before: u64,
    admitted_after: u64,
    capacity: usize,
) -> Result<ProductionReliableFlushTraceProjection, MergeSidecarError> {
    let evidence = admission.projection();
    let (connection_tenure_ordinal_high, connection_tenure_ordinal_low) =
        reliable_flush_ordinal_halves(evidence.connection_tenure_ordinal);
    let (delivery_ordinal_high, delivery_ordinal_low) =
        reliable_flush_ordinal_halves(evidence.delivery_ordinal);
    let message_cursor_before = reliable_flush_usize(evidence.message_cursor_before)?;
    let chunk_cursor_before = reliable_flush_usize(evidence.chunk_cursor_before)?;
    let (message_cursor_after, chunk_cursor_after) =
        if matches!(status, NetworkReplyFlushAckStatus::Flushed) {
            (
                reliable_flush_usize(evidence.message_cursor_after)?,
                reliable_flush_usize(evidence.chunk_cursor_after)?,
            )
        } else {
            (message_cursor_before, chunk_cursor_before)
        };
    Ok(ProductionReliableFlushTraceProjection {
        status: match status {
            NetworkReplyFlushAckStatus::Pending => 1,
            NetworkReplyFlushAckStatus::Flushed => 2,
            NetworkReplyFlushAckStatus::TimedOut | NetworkReplyFlushAckStatus::Closed => 3,
        },
        semantic_target: reliable_flush_peer_identity(&evidence.semantic_target),
        authenticated_source: reliable_flush_peer_identity(&evidence.authenticated_source),
        source_key_identity: reliable_flush_hash_identity(
            IDENTITY_DOMAIN_PROCESS_LOCAL,
            IDENTITY_KIND_REPLY_SOURCE_KEY,
            evidence.source_key_identity,
        ),
        delivery_route_identity: reliable_flush_hash_identity(
            IDENTITY_DOMAIN_PROCESS_LOCAL,
            IDENTITY_KIND_REPLY_DELIVERY_ROUTE,
            evidence.delivery_route_identity,
        ),
        writer_occurrence_identity: reliable_flush_hash_identity(
            IDENTITY_DOMAIN_PROCESS_LOCAL,
            IDENTITY_KIND_REPLY_WRITER_OCCURRENCE,
            evidence.writer_occurrence_identity,
        ),
        requester: reliable_flush_peer_identity(&evidence.requester),
        responder: reliable_flush_peer_identity(&evidence.responder),
        connection_tenure_ordinal_high,
        connection_tenure_ordinal_low,
        delivery_ordinal_high,
        delivery_ordinal_low,
        ticket_id: evidence.ticket_id,
        ticket_rank: reliable_flush_usize(evidence.ticket_rank)?,
        ticket_topic: reliable_flush_topic_tag(evidence.ticket_topic),
        reply_writer_timeout_attempt: evidence.reply_writer_timeout_attempt,
        canonical_request_digest: reliable_flush_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_REPLY_PAYLOAD,
            evidence.canonical_request_digest,
        ),
        stream_wire_bytes: reliable_flush_usize(evidence.stream_wire_bytes)?,
        request_id: reliable_flush_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_SIDECAR_REQUEST,
            evidence.request_id,
        ),
        service_generation: evidence.service_generation.get(),
        stream_epoch: evidence.stream_epoch.get(),
        semantic_sequence: evidence.semantic_sequence.get(),
        entry_hash: reliable_flush_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_MERGE_ENTRY,
            evidence.entry_hash,
        ),
        encoded_len: evidence.encoded_len,
        epoch_id: evidence.epoch_id,
        reference_digest: reliable_flush_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_REFERENCE_DIGEST,
            evidence.reference_digest,
        ),
        canonical_response_hash: reliable_flush_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_NETWORK_RESPONSE,
            evidence.canonical_response_hash,
        ),
        sidecar_response_hash: reliable_flush_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_SIDECAR_RESPONSE,
            evidence.sidecar_response_hash,
        ),
        chunk_hash: reliable_flush_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_SIDECAR_CHUNK,
            evidence.chunk_hash,
        ),
        payload_digest: reliable_flush_hash_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_SIDECAR_PAYLOAD,
            evidence.payload_digest,
        ),
        chunk_index: u64::from(evidence.chunk_index),
        chunk_count: u64::from(evidence.chunk_count),
        message_cursor_before,
        message_cursor_after,
        chunk_cursor_before,
        chunk_cursor_after,
        flushing_before,
        flushing_after,
        admitted_before,
        admitted_after,
        capacity: reliable_flush_usize(capacity)?,
    })
}
/// Move-only Sign command from a closed recovered carrier, rehashed against its
/// registry Effect; opaque request, marker, signature, and binding stay local.
#[must_use = "a recovered Sign task must enter its dedicated worker reservation"]
pub(in crate::sumeragi) struct RecoveredLifecycleSignTaskV1 {
    identity: RecoveredLifecycleSignDispatchIdentityV1,
    tag: EventTag,
    request: super::v2::SignRequest,
    prepared_candidate: Option<PreparedCandidateBody>,
}
impl RecoveredLifecycleSignTaskV1 {
    /// Seal exact carrier-derived material under its registry identity.
    pub(in crate::sumeragi) fn from_registry_projection(
        identity: RecoveredLifecycleSignDispatchIdentityV1,
        tag: EventTag,
        request: super::v2::SignRequest,
    ) -> Option<Self> {
        let prepared_candidate = match &request {
            super::v2::SignRequest::Vote(vote) if vote.phase == wire::GlobalPhase::Prepare => {
                Some(PreparedCandidateBody {
                    tag,
                    subject: vote.subject,
                })
            }
            super::v2::SignRequest::Proposal(_)
            | super::v2::SignRequest::Vote(_)
            | super::v2::SignRequest::TimeoutVote(_) => None,
        };
        identity.authorizes_request(tag, &request).then_some(Self {
            identity,
            tag,
            request,
            prepared_candidate,
        })
    }
    /// Return the dedicated class-sensitive queue key.
    pub(in crate::sumeragi) const fn dispatch_key(&self) -> RecoveredLifecycleSignDispatchKeyV1 {
        self.identity.key()
    }
    #[cfg(test)]
    fn for_test(
        ordinal: u128,
        tag: EventTag,
        request: super::v2::SignRequest,
        class: super::v2_lifecycle_coordinator::RecoveredLifecycleSignClassV1,
    ) -> Self {
        let identity =
            RecoveredLifecycleSignDispatchIdentityV1::for_test(ordinal, tag, &request, class)
                .expect("recovered Sign fixture retains one exact class and effect digest");
        Self::from_registry_projection(identity, tag, request)
            .expect("registry fixture identity revalidates its exact Sign material")
    }
}
/// Move-only recovered Decision Fetch authority joining the WAL-sealed registry
/// identity with private service signer/context fields.
#[must_use = "recovered Decision Fetch authority must enter the fixed production service"]
pub(in crate::sumeragi) struct RecoveredDecisionFetchRequestAuthorityV1 {
    identity: super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchIdentityV1,
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    sources: Vec<PeerId>,
    certificate: wire::QuorumCertificate,
}
impl RecoveredDecisionFetchRequestAuthorityV1 {
    /// Seal exact carrier material under its registry-minted identity.
    pub(in crate::sumeragi) fn from_registry_projection(
        identity: super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchIdentityV1,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        sources: Vec<PeerId>,
        certificate: wire::QuorumCertificate,
    ) -> Option<Self> {
        identity
            .authorizes_request(tag, round, subject, &sources, &certificate)
            .then_some(Self {
                identity,
                tag,
                round,
                subject,
                sources,
                certificate,
            })
    }
}
/// Dedicated executor owner for one lifecycle-recovered certified request.
///
/// No public or crate-visible constructor exists: only the fixed production
/// service method below can sign and authenticate the carrier-derived request.
#[must_use = "a recovered Decision Fetch owner must enter its dedicated executor index"]
pub(in crate::sumeragi) struct RecoveredDecisionFetchRequestOwnerV1 {
    key: super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1,
    tag: EventTag,
    sources: Vec<PeerId>,
    authenticated: AuthenticatedCertifiedBodyRequest,
    response_claim: Option<HashOf<wire::CertifiedBodyResponse>>,
}
impl RecoveredDecisionFetchRequestOwnerV1 {
    /// Build one exact dedicated owner from an already authenticated request.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        key: super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1,
        tag: EventTag,
        sources: Vec<PeerId>,
        authenticated: AuthenticatedCertifiedBodyRequest,
    ) -> Self {
        Self {
            key,
            tag,
            sources,
            authenticated,
            response_claim: None,
        }
    }
    /// Return the exact lifecycle request/response key.
    pub(in crate::sumeragi) const fn dispatch_key(
        &self,
    ) -> super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1 {
        self.key
    }
    /// Hash of the exact signed request family.
    pub(in crate::sumeragi) fn request_hash(&self) -> HashOf<wire::CertifiedBodyRequest> {
        self.authenticated.request_hash()
    }
    /// Compare the private transport logical request identity.
    pub(in crate::sumeragi) fn has_same_logical_identity(
        &self,
        request: &wire::CertifiedBodyRequest,
    ) -> bool {
        let owned = self.authenticated.request();
        owned.round == request.round
            && owned.subject == request.subject
            && owned.requester == request.requester
    }
    /// Ask the ordinary tracker whether it already owns this private request identity.
    pub(in crate::sumeragi) fn conflicts_with_ordinary_tracker(
        &self,
        tracker: &super::v2_transport::OutstandingCertifiedBodyRequests,
    ) -> bool {
        tracker.contains_authenticated_identity(&self.authenticated)
    }
    /// Compare body coordinates without exposing the retained signed request.
    pub(in crate::sumeragi) fn matches_body_coordinates(
        &self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> bool {
        self.authenticated.request().round == round
            && self.authenticated.request().subject == subject
    }
    /// Recheck the exact dedicated executor height and requester.
    pub(in crate::sumeragi) fn validates_exact_executor_context(
        &self,
        context: &wire::HeightContext,
        requester: &PeerId,
    ) -> bool {
        let request = self.authenticated.request();
        self.key.matches_height_context(context)
            && self.tag.height() == context.height
            && request.round.context_id == context.id()
            && request.round.height == context.height
            && &request.requester == requester
            && self.request_hash() == HashOf::new(request)
            && self.sources
                == context
                    .roster
                    .iter()
                    .map(|entry| entry.validator.clone())
                    .collect::<Vec<_>>()
    }
    /// Authenticate one response against this exact request without exposing it.
    pub(in crate::sumeragi) fn authenticate_response(
        &self,
        context: &wire::HeightContext,
        response: wire::CertifiedBodyResponse,
        authenticated_responder: &PeerId,
    ) -> Result<super::v2_transport::AuthenticatedCertifiedBodyResponse, V2TransportError> {
        self.authenticated
            .authenticate_response(context, response, authenticated_responder)
    }
    /// Project only the immutable candidate coordinates needed by the selector.
    pub(in crate::sumeragi) const fn candidate_projection(
        &self,
    ) -> RecoveredDecisionFetchOwnerCandidateProjectionV1 {
        RecoveredDecisionFetchOwnerCandidateProjectionV1 {
            tag: self.tag,
            round: self.authenticated.request().round,
            subject: self.authenticated.request().subject,
            response_claim: self.response_claim,
        }
    }
    /// Recheck the exact claimed response before post-publication owner retirement.
    pub(in crate::sumeragi) fn matches_settlement(
        &self,
        key: super::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1,
        response_hash: HashOf<wire::CertifiedBodyResponse>,
    ) -> bool {
        self.key == key && self.response_claim == Some(response_hash)
    }
    /// Recheck the request-scoped response claim observed by a sealed selector candidate.
    pub(in crate::sumeragi) fn matches_response_claim_preflight(
        &self,
        response_hash: HashOf<wire::CertifiedBodyResponse>,
        expected: super::v2_transport::CertifiedBodyResponseClaimPreflight,
    ) -> bool {
        match (self.response_claim, expected) {
            (None, super::v2_transport::CertifiedBodyResponseClaimPreflight::Vacant) => true,
            (
                Some(claimed),
                super::v2_transport::CertifiedBodyResponseClaimPreflight::ExactRetransmission,
            ) => claimed == response_hash,
            (Some(_), super::v2_transport::CertifiedBodyResponseClaimPreflight::Vacant)
            | (
                None,
                super::v2_transport::CertifiedBodyResponseClaimPreflight::ExactRetransmission,
            ) => false,
        }
    }
    /// Install or coalesce only the exact response hash which was revalidated
    /// immediately before the dedicated queue publication tail.
    pub(in crate::sumeragi) fn commit_exact_response_claim(
        &mut self,
        response_hash: HashOf<wire::CertifiedBodyResponse>,
    ) -> bool {
        match self.response_claim {
            None => {
                self.response_claim = Some(response_hash);
                true
            }
            Some(claimed) => claimed == response_hash,
        }
    }
}
/// Minimal immutable selector projection of a dedicated recovered request owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) struct RecoveredDecisionFetchOwnerCandidateProjectionV1 {
    pub(in crate::sumeragi) tag: EventTag,
    pub(in crate::sumeragi) round: wire::ConsensusRound,
    pub(in crate::sumeragi) subject: wire::BlockSubject,
    pub(in crate::sumeragi) response_claim: Option<HashOf<wire::CertifiedBodyResponse>>,
}
/// Closed signed output retaining its task, successor material, Proposal bytes,
/// and signature opaquely for the restart-closed successor transaction.
struct RecoveredLifecycleSignWorkerResultV1 {
    task: RecoveredLifecycleSignTaskV1,
    signature: Vec<u8>,
    outbound_payload: Option<EncodedV2Payload>,
}
/// Move-only worker proof whose private one-shot projection lets only the
/// recovered-Sign adapter replay `Signed` and expose no raw material.
#[must_use = "recovered Sign material must enter the fixed adapter preview"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct RecoveredLifecycleSignAdapterCompletionAuthorityV1 {
    key: RecoveredLifecycleSignDispatchKeyV1,
    tag: EventTag,
    request: super::v2::SignRequest,
    signature: Vec<u8>,
    outbound_payload: Option<EncodedV2Payload>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredLifecycleSignAdapterCompletionAuthorityV1 {
    /// Consume the worker proof through the adapter's private projection seam.
    pub(in crate::sumeragi) fn consume_for_adapter(
        self,
        _permit: super::v2::RecoveredLifecycleSignAdapterCompletionPermitV1,
    ) -> (
        RecoveredLifecycleSignDispatchKeyV1,
        EventTag,
        super::v2::SignRequest,
        Vec<u8>,
        Option<EncodedV2Payload>,
    ) {
        (
            self.key,
            self.tag,
            self.request,
            self.signature,
            self.outbound_payload,
        )
    }
    /// Build exact guarded Sign material for adapter-preview behavior tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn for_test(
        ordinal: u128,
        tag: EventTag,
        request: super::v2::SignRequest,
        signature: Vec<u8>,
        outbound_payload: Option<EncodedV2Payload>,
        class: super::v2_lifecycle_coordinator::RecoveredLifecycleSignClassV1,
    ) -> Self {
        let task = RecoveredLifecycleSignTaskV1::for_test(ordinal, tag, request, class);
        Self {
            key: task.dispatch_key(),
            tag: task.tag,
            request: task.request,
            signature,
            outbound_payload,
        }
    }
}
impl RecoveredLifecycleSignWorkerResultV1 {
    const fn dispatch_key(&self) -> RecoveredLifecycleSignDispatchKeyV1 {
        self.task.dispatch_key()
    }
    fn is_exact(&self) -> bool {
        let expected_prepared = match &self.task.request {
            super::v2::SignRequest::Vote(vote) if vote.phase == wire::GlobalPhase::Prepare => {
                Some(PreparedCandidateBody {
                    tag: self.task.tag,
                    subject: vote.subject,
                })
            }
            super::v2::SignRequest::Proposal(_)
            | super::v2::SignRequest::Vote(_)
            | super::v2::SignRequest::TimeoutVote(_) => None,
        };
        self.task
            .identity
            .authorizes_request(self.task.tag, &self.task.request)
            && self.task.prepared_candidate == expected_prepared
            && !self.signature.is_empty()
            && match (&self.task.request, &self.outbound_payload) {
                (super::v2::SignRequest::Proposal(proposal), Some(payload)) => {
                    payload.manifest() == &proposal.manifest
                }
                (
                    super::v2::SignRequest::Vote(_) | super::v2::SignRequest::TimeoutVote(_),
                    None,
                ) => true,
                (
                    super::v2::SignRequest::Proposal(_)
                    | super::v2::SignRequest::Vote(_)
                    | super::v2::SignRequest::TimeoutVote(_),
                    _,
                ) => false,
            }
    }
}
/// Dedicated lifecycle-owned Certified-Serve command.
///
/// The claimed lease, authenticated request, and exact reply capability remain
/// inseparable from physical dequeue through worker completion.
pub(in crate::sumeragi) struct LifecycleCertifiedServeTaskV1 {
    authority: Option<LifecycleCertifiedServeTaskAuthorityV1>,
    lifecycle_ordinal: u128,
    authenticated: AuthenticatedCertifiedBodyRequest,
    recipient: PeerId,
    reply_routes: NetworkReplyRoutes,
    ingress_ownership: FairV2IngressOwnershipEvidence,
}
enum LifecycleCertifiedServeTaskAuthorityV1 {
    Claimed(TurnLease),
    TerminalReplay(CertifiedServeTerminalReplayAuthorizationV1),
}
impl LifecycleCertifiedServeTaskV1 {
    /// Join one claimed lifecycle lease to the exact physically dequeued request.
    ///
    /// # Errors
    ///
    /// Returns an error when the carrier, requester, reply route, or ingress
    /// ownership no longer matches the authenticated lifecycle request.
    pub(in crate::sumeragi) fn from_dequeued(
        dispatch: ClaimedCertifiedServeDispatchV1,
        inbound: InboundBlockMessage,
    ) -> Result<Self, String> {
        let (lease, authenticated) = dispatch.into_worker_parts();
        let lifecycle_ordinal = lease.ordinal();
        Self::from_dequeued_parts(
            LifecycleCertifiedServeTaskAuthorityV1::Claimed(lease),
            lifecycle_ordinal,
            authenticated,
            inbound,
        )
    }

    /// Join one terminal replay authority to the exact physically dequeued request.
    ///
    /// # Errors
    ///
    /// Returns an error when the replay authority or transport carrier no longer
    /// names the authenticated terminal request.
    pub(in crate::sumeragi) fn from_terminal_replay(
        authorization: CertifiedServeTerminalReplayAuthorizationV1,
        authenticated: AuthenticatedCertifiedBodyRequest,
        inbound: InboundBlockMessage,
    ) -> Result<Self, String> {
        if !authorization.authorizes_request(&authenticated) {
            return Err("terminal Certified-Serve replay changed its request".to_owned());
        }
        let lifecycle_ordinal = authorization.ordinal();
        Self::from_dequeued_parts(
            LifecycleCertifiedServeTaskAuthorityV1::TerminalReplay(authorization),
            lifecycle_ordinal,
            authenticated,
            inbound,
        )
    }

    fn from_dequeued_parts(
        authority: LifecycleCertifiedServeTaskAuthorityV1,
        lifecycle_ordinal: u128,
        authenticated: AuthenticatedCertifiedBodyRequest,
        mut inbound: InboundBlockMessage,
    ) -> Result<Self, String> {
        let BlockMessage::V2(message) = inbound.message() else {
            return Err("claimed Certified-Serve dequeue lost its v2 carrier".to_owned());
        };
        let wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) = &message.payload
        else {
            return Err("claimed Certified-Serve dequeue changed its request family".to_owned());
        };
        let Some(recipient) = inbound.sender().cloned() else {
            return Err("claimed Certified-Serve dequeue lost its requester".to_owned());
        };
        let Some(routes) = inbound.reply_routes() else {
            return Err("claimed Certified-Serve dequeue lost its reply routes".to_owned());
        };
        let Some(ownership) = inbound.ingress_ownership() else {
            return Err("claimed Certified-Serve dequeue lost its ingress ownership".to_owned());
        };
        if request != authenticated.request()
            || HashOf::new(request) != authenticated.request_hash()
            || &recipient != &authenticated.request().requester
            || routes.semantic_target() != &recipient
            || !ownership.validate_exact()
            || !ownership.matches_message(inbound.message())
            || !ownership.matches_semantic_origin(Some(&recipient))
            || !ownership.matches_reply_routes(Some(routes))
        {
            return Err(
                "claimed Certified-Serve dequeue changed exact transport ownership".to_owned(),
            );
        }
        let ingress_ownership = inbound
            .take_ingress_ownership()
            .expect("validated Certified-Serve ingress ownership remains present");
        let (_, sender, reply_routes) = inbound.into_message_sender_and_reply_routes();
        Ok(Self {
            authority: Some(authority),
            lifecycle_ordinal,
            authenticated,
            recipient: sender.expect("validated Certified-Serve requester remains present"),
            reply_routes: reply_routes
                .expect("validated Certified-Serve reply routes remain present"),
            ingress_ownership,
        })
    }

    const fn lifecycle_ordinal(&self) -> u128 {
        self.lifecycle_ordinal
    }

    fn request_hash(&self) -> HashOf<wire::CertifiedBodyRequest> {
        self.authenticated.request_hash()
    }

    fn authority_matches_request(&self) -> bool {
        match self.authority.as_ref() {
            Some(LifecycleCertifiedServeTaskAuthorityV1::Claimed(lease)) => {
                lease.work_class()
                    == super::v2_lifecycle_coordinator::LifecycleWorkClass::CertifiedServe
                    && lease.ordinal() == self.lifecycle_ordinal
            }
            Some(LifecycleCertifiedServeTaskAuthorityV1::TerminalReplay(authorization)) => {
                authorization.ordinal() == self.lifecycle_ordinal
                    && authorization.authorizes_request(&self.authenticated)
            }
            None => false,
        }
    }
}
/// Exact worker result retained until LedgerV1 terminal publication and reply delivery.
struct LifecycleCertifiedServeWorkerResultV1 {
    task: LifecycleCertifiedServeTaskV1,
    body_readback: Option<DurableCertifiedServeBodyReadbackV1>,
    response: wire::CertifiedBodyResponse,
}
impl LifecycleCertifiedServeWorkerResultV1 {
    const fn lifecycle_ordinal(&self) -> u128 {
        self.task.lifecycle_ordinal()
    }

    fn request_hash(&self) -> HashOf<wire::CertifiedBodyRequest> {
        self.task.request_hash()
    }
}
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum V2IoCommand {
    Sign {
        task: ConsensusSignTask,
        restore_outbound_payload: bool,
    },
    Store(BodyStoreTask),
    PersistCertifiedFetchBody(CertifiedFetchBodyPersistenceTask),
    PersistRecoveredDecisionFetchBody(RecoveredDecisionFetchBodyPersistenceTaskV1),
    Validate(BodyValidationTask),
    Apply(ApplyTask),
    RecoveredDecisionApply(RecoveredDecisionApplyTaskV1),
    RecoveredLifecycleSign(RecoveredLifecycleSignTaskV1),
    #[cfg(test)]
    RecoveredDecisionApplyFixture(RecoveredDecisionApplyDispatchKeyV1),
    LifecycleCertifiedServe(LifecycleCertifiedServeTaskV1),
    LoadCandidate {
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
    },
    Retire(V2RetireCommand),
    Shutdown,
}
struct V2RetireCommand {
    receipt: KuraV2CommitReceipt,
    cleanup: V2CleanupSubmission,
    chunk_root: PathBuf,
}
const LOCAL_IO_CONTROL_RESERVE: usize = 1;
const CERTIFIED_SERVE_PHASE_FAMILIES: usize = 2;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum V2IoAdmissionClass {
    Auxiliary,
    Consensus,
    Control,
}
impl V2IoCommand {
    const fn admission_class(&self) -> V2IoAdmissionClass {
        match self {
            Self::LifecycleCertifiedServe(_) => V2IoAdmissionClass::Auxiliary,
            Self::Sign { .. }
            | Self::Store(_)
            | Self::PersistCertifiedFetchBody(_)
            | Self::PersistRecoveredDecisionFetchBody(_)
            | Self::Validate(_)
            | Self::Apply(_)
            | Self::RecoveredDecisionApply(_)
            | Self::RecoveredLifecycleSign(_) => V2IoAdmissionClass::Consensus,
            #[cfg(test)]
            Self::RecoveredDecisionApplyFixture(_) => V2IoAdmissionClass::Consensus,
            Self::LoadCandidate { .. } | Self::Retire(_) | Self::Shutdown => {
                V2IoAdmissionClass::Control
            }
        }
    }
    const fn work_id(&self) -> Option<EffectWorkId> {
        match self {
            Self::Sign { task, .. } => Some(task.id()),
            Self::Store(task) => Some(task.id()),
            Self::PersistCertifiedFetchBody(task) => Some(task.work_id()),
            Self::Validate(task) => Some(task.id()),
            Self::Apply(task) => Some(task.id()),
            Self::RecoveredDecisionApply(_)
            | Self::RecoveredLifecycleSign(_)
            | Self::PersistRecoveredDecisionFetchBody(_)
            | Self::LifecycleCertifiedServe(_)
            | Self::LoadCandidate { .. }
            | Self::Retire(_)
            | Self::Shutdown => None,
            #[cfg(test)]
            Self::RecoveredDecisionApplyFixture(_) => None,
        }
    }
    /// Runtime lifecycle retained by a completion-producing consensus command.
    const fn runtime_lifecycle_ordinal(&self) -> Option<u128> {
        match self {
            Self::Sign { task, .. } => Some(task.lifecycle_ordinal()),
            Self::Store(task) => Some(task.lifecycle_ordinal()),
            Self::Validate(task) => Some(task.lifecycle_ordinal()),
            Self::Apply(task) => Some(task.lifecycle_ordinal()),
            Self::RecoveredDecisionApply(task) => Some(task.dispatch_key().lifecycle_ordinal()),
            Self::RecoveredLifecycleSign(task) => Some(task.dispatch_key().lifecycle_ordinal()),
            Self::PersistRecoveredDecisionFetchBody(task) => {
                Some(task.dispatch_key().lifecycle_ordinal())
            }
            Self::LifecycleCertifiedServe(task) => Some(task.lifecycle_ordinal()),
            #[cfg(test)]
            Self::RecoveredDecisionApplyFixture(key) => Some(key.lifecycle_ordinal()),
            Self::PersistCertifiedFetchBody(_)
            | Self::LoadCandidate { .. }
            | Self::Retire(_)
            | Self::Shutdown => None,
        }
    }
    const fn lifecycle_certified_serve_ordinal(&self) -> Option<u128> {
        match self {
            Self::LifecycleCertifiedServe(task) => Some(task.lifecycle_ordinal()),
            Self::Sign { .. }
            | Self::Store(_)
            | Self::PersistCertifiedFetchBody(_)
            | Self::PersistRecoveredDecisionFetchBody(_)
            | Self::Validate(_)
            | Self::Apply(_)
            | Self::RecoveredDecisionApply(_)
            | Self::RecoveredLifecycleSign(_)
            | Self::LoadCandidate { .. }
            | Self::Retire(_)
            | Self::Shutdown => None,
            #[cfg(test)]
            Self::RecoveredDecisionApplyFixture(_) => None,
        }
    }
    const fn cancellable_kind(&self) -> Option<V2IoCancellableKind> {
        match self {
            Self::Sign { .. } => Some(V2IoCancellableKind::Sign),
            Self::Store(_) => Some(V2IoCancellableKind::Store),
            Self::Validate(_) => Some(V2IoCancellableKind::Validate),
            Self::Apply(_)
            | Self::RecoveredDecisionApply(_)
            | Self::RecoveredLifecycleSign(_)
            | Self::PersistCertifiedFetchBody(_)
            | Self::PersistRecoveredDecisionFetchBody(_)
            | Self::LifecycleCertifiedServe(_)
            | Self::LoadCandidate { .. }
            | Self::Retire(_)
            | Self::Shutdown => None,
            #[cfg(test)]
            Self::RecoveredDecisionApplyFixture(_) => None,
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
            Self::PersistCertifiedFetchBody(task) => Some((
                task.work_id(),
                V2IoWorkDescriptor::PersistCertifiedFetchBody {
                    id: task.id(),
                    response_hash: task.response_hash(),
                },
            )),
            Self::Validate(task) => Some((
                task.id(),
                V2IoWorkDescriptor::Validate {
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
            Self::RecoveredDecisionApply(_)
            | Self::RecoveredLifecycleSign(_)
            | Self::PersistRecoveredDecisionFetchBody(_)
            | Self::LifecycleCertifiedServe(_)
            | Self::LoadCandidate { .. }
            | Self::Retire(_)
            | Self::Shutdown => None,
            #[cfg(test)]
            Self::RecoveredDecisionApplyFixture(_) => None,
        }
    }
    const fn recovered_decision_apply_key(&self) -> Option<RecoveredDecisionApplyDispatchKeyV1> {
        match self {
            Self::RecoveredDecisionApply(task) => Some(task.dispatch_key()),
            #[cfg(test)]
            Self::RecoveredDecisionApplyFixture(key) => Some(*key),
            Self::Sign { .. }
            | Self::Store(_)
            | Self::PersistCertifiedFetchBody(_)
            | Self::PersistRecoveredDecisionFetchBody(_)
            | Self::Validate(_)
            | Self::Apply(_)
            | Self::RecoveredLifecycleSign(_)
            | Self::LifecycleCertifiedServe(_)
            | Self::LoadCandidate { .. }
            | Self::Retire(_)
            | Self::Shutdown => None,
        }
    }
    const fn recovered_lifecycle_sign_key(&self) -> Option<RecoveredLifecycleSignDispatchKeyV1> {
        match self {
            Self::RecoveredLifecycleSign(task) => Some(task.dispatch_key()),
            Self::Sign { .. }
            | Self::Store(_)
            | Self::PersistCertifiedFetchBody(_)
            | Self::PersistRecoveredDecisionFetchBody(_)
            | Self::Validate(_)
            | Self::Apply(_)
            | Self::RecoveredDecisionApply(_)
            | Self::LifecycleCertifiedServe(_)
            | Self::LoadCandidate { .. }
            | Self::Retire(_)
            | Self::Shutdown => None,
            #[cfg(test)]
            Self::RecoveredDecisionApplyFixture(_) => None,
        }
    }
    const fn recovered_decision_fetch_key(&self) -> Option<RecoveredDecisionFetchDispatchKeyV1> {
        match self {
            Self::PersistRecoveredDecisionFetchBody(task) => Some(task.dispatch_key()),
            Self::Sign { .. }
            | Self::Store(_)
            | Self::PersistCertifiedFetchBody(_)
            | Self::Validate(_)
            | Self::Apply(_)
            | Self::RecoveredDecisionApply(_)
            | Self::RecoveredLifecycleSign(_)
            | Self::LifecycleCertifiedServe(_)
            | Self::LoadCandidate { .. }
            | Self::Retire(_)
            | Self::Shutdown => None,
            #[cfg(test)]
            Self::RecoveredDecisionApplyFixture(_) => None,
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
    PersistCertifiedFetchBody {
        id: CertifiedFetchBodyPersistenceId,
        response_hash: HashOf<wire::CertifiedBodyResponse>,
    },
    Validate {
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
            Self::PersistCertifiedFetchBody { .. } | Self::Apply { .. } => None,
        }
    }
}
/// Hierarchical single-FIFO admission: auxiliary work uses its prefix,
/// consensus its suffix, and trusted control the final slot without reordering.
struct V2IoAdmission {
    queued: AtomicUsize,
    lifecycle_capacity_generation: AtomicU64,
    lifecycle_capacity_generation_exhausted: AtomicBool,
    auxiliary_limit: usize,
    consensus_limit: usize,
    capacity: usize,
    completion_capacity: usize,
    completion_state: Mutex<V2IoCompletionQueueState>,
}
#[derive(Clone, Copy, Debug)]
struct V2IoCompletionOwnership {
    retained_at: Instant,
    service_debt: u64,
    requires_runtime_capacity: bool,
    runtime_lifecycle_ordinal: Option<u128>,
    recovered_decision_apply: Option<RecoveredDecisionApplyDispatchKeyV1>,
    recovered_lifecycle_sign: Option<RecoveredLifecycleSignDispatchKeyV1>,
    recovered_decision_fetch: Option<RecoveredDecisionFetchDispatchKeyV1>,
    lifecycle_certified_serve: Option<u128>,
}
#[derive(Debug, Default)]
struct V2IoCompletionQueueState {
    owned: VecDeque<V2IoCompletionOwnership>,
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
            lifecycle_capacity_generation: AtomicU64::new(0),
            lifecycle_capacity_generation_exhausted: AtomicBool::new(false),
            auxiliary_limit: auxiliary_capacity,
            consensus_limit,
            capacity,
            // A synchronous channel can buffer `capacity` results while its
            // single ordered producer retains one more completed result in a
            // blocked `send`. The serialized consumer may additionally hold
            // one runtime-producing result while it drains auxiliary results
            // behind a full reducer FIFO. All three owners remain bounded.
            completion_capacity: capacity.saturating_add(2),
            completion_state: Mutex::new(V2IoCompletionQueueState::default()),
        })
    }
    #[cfg(test)]
    fn unbounded_for_tests() -> Arc<Self> {
        Arc::new(Self {
            queued: AtomicUsize::new(0),
            lifecycle_capacity_generation: AtomicU64::new(0),
            lifecycle_capacity_generation_exhausted: AtomicBool::new(false),
            auxiliary_limit: usize::MAX,
            consensus_limit: usize::MAX,
            capacity: usize::MAX,
            completion_capacity: usize::MAX,
            completion_state: Mutex::new(V2IoCompletionQueueState::default()),
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
    /// Return the exact physical admission count while the queue state is locked.
    fn queued(&self) -> usize {
        self.queued.load(AtomicOrdering::Acquire)
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
        if self
            .lifecycle_capacity_generation
            .fetch_update(
                AtomicOrdering::AcqRel,
                AtomicOrdering::Acquire,
                |generation| generation.checked_add(1),
            )
            .is_err()
        {
            self.lifecycle_capacity_generation_exhausted
                .store(true, AtomicOrdering::Release);
        }
    }
    fn lifecycle_capacity_generation(&self) -> u64 {
        self.lifecycle_capacity_generation
            .load(AtomicOrdering::Acquire)
    }
    fn lifecycle_capacity_generation_exhausted(&self) -> bool {
        self.lifecycle_capacity_generation_exhausted
            .load(AtomicOrdering::Acquire)
    }
    fn retain_completion(
        &self,
        retained_at: Instant,
        requires_runtime_capacity: bool,
        runtime_lifecycle_ordinal: Option<u128>,
        recovered_decision_apply: Option<RecoveredDecisionApplyDispatchKeyV1>,
        recovered_lifecycle_sign: Option<RecoveredLifecycleSignDispatchKeyV1>,
        recovered_decision_fetch: Option<RecoveredDecisionFetchDispatchKeyV1>,
        lifecycle_certified_serve: Option<u128>,
    ) {
        let mut state = self
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert!(
            state.owned.len() < self.completion_capacity,
            "Sumeragi v2 I/O worker exceeded bounded completion ownership"
        );
        state.owned.push_back(V2IoCompletionOwnership {
            retained_at,
            service_debt: 0,
            requires_runtime_capacity,
            runtime_lifecycle_ordinal,
            recovered_decision_apply,
            recovered_lifecycle_sign,
            recovered_decision_fetch,
            lifecycle_certified_serve,
        });
    }
    fn abandon_latest_completion(&self) {
        let mut state = self
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state
            .owned
            .pop_back()
            .expect("failed completion send must retain its ownership record");
    }
    fn acknowledge_completion_at(&self, position: usize) {
        let mut state = self
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Some unit seams inject directly into the raw channel. Production
        // sends always retain an ownership record before publication.
        let _ = state.owned.remove(position);
    }
    fn recovered_decision_apply_completion_is_exact(
        &self,
        key: RecoveredDecisionApplyDispatchKeyV1,
    ) -> bool {
        let state = self
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut matches = state.owned.iter().filter(|owned| {
            owned.requires_runtime_capacity
                && owned.runtime_lifecycle_ordinal == Some(key.lifecycle_ordinal())
                && owned.recovered_decision_apply == Some(key)
        });
        matches.next().is_some() && matches.next().is_none()
    }
    fn transfer_recovered_lifecycle_sign_completion_at(
        &self,
        key: RecoveredLifecycleSignDispatchKeyV1,
        position: usize,
    ) -> bool {
        let mut state = self
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(owner) = state.owned.get(position) else {
            return false;
        };
        if !owner.requires_runtime_capacity
            || owner.runtime_lifecycle_ordinal != Some(key.lifecycle_ordinal())
            || owner.recovered_lifecycle_sign != Some(key)
            || state
                .owned
                .iter()
                .filter(|owned| owned.recovered_lifecycle_sign == Some(key))
                .count()
                != 1
        {
            return false;
        }
        state.owned.remove(position).is_some()
    }
    fn transfer_recovered_decision_fetch_completion_at(
        &self,
        key: RecoveredDecisionFetchDispatchKeyV1,
        position: usize,
    ) -> bool {
        let mut state = self
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(owner) = state.owned.get(position) else {
            return false;
        };
        if owner.requires_runtime_capacity
            || owner.runtime_lifecycle_ordinal != Some(key.lifecycle_ordinal())
            || owner.recovered_decision_fetch != Some(key)
            || state
                .owned
                .iter()
                .filter(|owned| owned.recovered_decision_fetch == Some(key))
                .count()
                != 1
        {
            return false;
        }
        state.owned.remove(position).is_some()
    }
    fn transfer_lifecycle_certified_serve_completion_at(
        &self,
        ordinal: u128,
        position: usize,
    ) -> bool {
        let mut state = self
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(owner) = state.owned.get(position) else {
            return false;
        };
        if owner.requires_runtime_capacity
            || owner.runtime_lifecycle_ordinal != Some(ordinal)
            || owner.recovered_decision_apply.is_some()
            || owner.recovered_lifecycle_sign.is_some()
            || owner.recovered_decision_fetch.is_some()
            || owner.lifecycle_certified_serve != Some(ordinal)
            || state
                .owned
                .iter()
                .filter(|owned| owned.lifecycle_certified_serve == Some(ordinal))
                .count()
                != 1
        {
            return false;
        }
        state.owned.remove(position).is_some()
    }
    fn transfer_recovered_decision_apply_completion(
        &self,
        key: RecoveredDecisionApplyDispatchKeyV1,
    ) -> bool {
        let mut state = self
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut positions = state
            .owned
            .iter()
            .enumerate()
            .filter_map(|(position, owned)| {
                (owned.requires_runtime_capacity
                    && owned.runtime_lifecycle_ordinal == Some(key.lifecycle_ordinal())
                    && owned.recovered_decision_apply == Some(key))
                .then_some(position)
            });
        let Some(position) = positions.next() else {
            return false;
        };
        if positions.next().is_some() {
            return false;
        }
        state.owned.remove(position).is_some()
    }
    fn acknowledge_recovered_decision_apply_completion(
        &self,
        key: RecoveredDecisionApplyDispatchKeyV1,
    ) {
        assert!(
            self.transfer_recovered_decision_apply_completion(key),
            "settled recovered Apply must retain one exact completion owner"
        );
    }
    fn completion_requires_runtime_capacity_at(&self, position: usize) -> Option<bool> {
        self.completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .owned
            .get(position)
            .map(|owned| owned.requires_runtime_capacity)
    }
    fn completion_ownership_at(&self, position: usize) -> Option<V2IoCompletionOwnership> {
        self.completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .owned
            .get(position)
            .copied()
    }
    fn record_completion_service_debt(&self) -> bool {
        let mut state = self
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(oldest) = state.owned.front_mut() else {
            return false;
        };
        oldest.service_debt = oldest.service_debt.saturating_add(1);
        true
    }
    fn completion_snapshot(&self, now: Instant) -> RuntimeQueueLaneSnapshot {
        let state = self
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let oldest = state.owned.front();
        RuntimeQueueLaneSnapshot {
            depth: state.owned.len(),
            capacity: self.completion_capacity,
            oldest_age: oldest.map(|owned| now.saturating_duration_since(owned.retained_at)),
            max_service_debt: oldest.map_or(0, |owned| owned.service_debt),
        }
    }
}
impl super::status::V2IoCompletionQueueObserver for V2IoAdmission {
    fn completion_queue_snapshot(&self, now: Instant) -> RuntimeQueueLaneSnapshot {
        self.completion_snapshot(now)
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
#[derive(Debug)]
struct V2IoTrackedRecoveredDecisionApplyV1 {
    state: V2IoWorkState,
}
#[derive(Debug)]
struct V2IoTrackedRecoveredLifecycleSignV1 {
    state: V2IoWorkState,
}
#[derive(Debug)]
struct V2IoTrackedRecoveredDecisionFetchBodyV1 {
    id: super::v2_lifecycle_coordinator::RecoveredDecisionFetchBodyPersistenceIdV1,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    state: V2IoWorkState,
}
enum RecoveredDecisionApplyRetryQueueErrorV1<T> {
    Unavailable(T),
    InvalidOwner(T),
}
trait RecoveredDecisionApplyRetryTaskV1 {
    fn dispatch_key(&self) -> RecoveredDecisionApplyDispatchKeyV1;
    fn into_command(self) -> V2IoCommand;
}
impl RecoveredDecisionApplyRetryTaskV1 for RecoveredDecisionApplyTaskV1 {
    fn dispatch_key(&self) -> RecoveredDecisionApplyDispatchKeyV1 {
        RecoveredDecisionApplyTaskV1::dispatch_key(self)
    }
    fn into_command(self) -> V2IoCommand {
        V2IoCommand::RecoveredDecisionApply(self)
    }
}
#[cfg(test)]
struct RecoveredDecisionApplyRetryTaskFixtureV1(RecoveredDecisionApplyDispatchKeyV1);
#[cfg(test)]
impl RecoveredDecisionApplyRetryTaskV1 for RecoveredDecisionApplyRetryTaskFixtureV1 {
    fn dispatch_key(&self) -> RecoveredDecisionApplyDispatchKeyV1 {
        self.0
    }
    fn into_command(self) -> V2IoCommand {
        V2IoCommand::RecoveredDecisionApplyFixture(self.0)
    }
}
#[derive(Debug)]
struct V2IoTrackedLifecycleServeV1 {
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    state: V2IoWorkState,
}
struct V2IoCommandQueueState {
    commands: VecDeque<V2IoCommand>,
    work: BTreeMap<EffectWorkId, V2IoTrackedWork>,
    recovered_decision_applies:
        BTreeMap<RecoveredDecisionApplyDispatchKeyV1, V2IoTrackedRecoveredDecisionApplyV1>,
    recovered_lifecycle_signs:
        BTreeMap<RecoveredLifecycleSignDispatchKeyV1, V2IoTrackedRecoveredLifecycleSignV1>,
    recovered_decision_fetch_bodies:
        BTreeMap<RecoveredDecisionFetchDispatchKeyV1, V2IoTrackedRecoveredDecisionFetchBodyV1>,
    lifecycle_serves: BTreeMap<u128, V2IoTrackedLifecycleServeV1>,
    sender_open: bool,
    receiver_open: bool,
}
/// Bounded cancellable reducer/I/O FIFO whose indexed ownership survives active
/// work and pending delivery, making retransmission idempotent without debt.
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
/// Service-owned release-generation observation for one unavailable lifecycle
/// target. It exposes neither queue depth nor an admission limit; retry is
/// meaningful only after the same queue reports that a real release advanced
/// this opaque observation.
#[must_use = "capacity retry must wait for the service generation to advance"]
pub(crate) struct LifecycleIoCapacityWait {
    queue: Arc<V2IoCommandQueue>,
    output_guard: Arc<ConsensusOutputGuard>,
    target: LifecycleIngressIoTargetSeal,
    observed_generation: u64,
}
/// Sealed liveness classification for one retained lifecycle capacity wait.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LifecycleIoCapacityWaitStatus {
    /// The same connected service has not released capacity since capture.
    SamePending,
    /// The same connected service observed at least one real capacity release.
    Released,
    /// The same service reached the terminal release generation.
    GenerationExhausted,
    /// The retained authority no longer names a live matching service.
    ForeignOrDisconnected,
}
impl LifecycleIoCapacityWait {
    /// Classify retry liveness against the exact service which minted the wait.
    pub(crate) fn status(&self, services: &ProductionV2Services) -> LifecycleIoCapacityWaitStatus {
        let target_context = self.target.context();
        let Some(io) = services.io.as_ref() else {
            return LifecycleIoCapacityWaitStatus::ForeignOrDisconnected;
        };
        if !Arc::ptr_eq(&self.queue, &io.command_tx.queue)
            || !Arc::ptr_eq(&self.output_guard, &services.output_guard)
            || target_context.height() != services.context.height
            || target_context.id().as_bytes() != services.context.id().0.as_ref()
            || services.output_guard.restart_required()
        {
            return LifecycleIoCapacityWaitStatus::ForeignOrDisconnected;
        }
        let state = self.queue.lock();
        if !state.sender_open || !state.receiver_open {
            return LifecycleIoCapacityWaitStatus::ForeignOrDisconnected;
        }
        if self
            .queue
            .admission
            .lifecycle_capacity_generation_exhausted()
        {
            return LifecycleIoCapacityWaitStatus::GenerationExhausted;
        }
        let status = match self
            .queue
            .admission
            .lifecycle_capacity_generation()
            .cmp(&self.observed_generation)
        {
            std::cmp::Ordering::Greater => LifecycleIoCapacityWaitStatus::Released,
            std::cmp::Ordering::Equal => LifecycleIoCapacityWaitStatus::SamePending,
            std::cmp::Ordering::Less => LifecycleIoCapacityWaitStatus::ForeignOrDisconnected,
        };
        if services.output_guard.restart_required() {
            LifecycleIoCapacityWaitStatus::ForeignOrDisconnected
        } else {
            status
        }
    }
}
/// Borrow-bound target reservation holding queue state and admission together.
/// It permits only typed abort or post-preflight consumption into `target`'s
/// command family; any other Drop closes output before releasing the slot.
#[must_use = "the exact I/O reservation must commit or use its typed pre-plan abort"]
pub(crate) struct LifecycleIoCapacityReservation<'a> {
    queue: &'a V2IoCommandQueue,
    state: Option<std::sync::MutexGuard<'a, V2IoCommandQueueState>>,
    operation: Option<ConsensusFailStopOperation<'a>>,
    target: Option<LifecycleIngressIoTargetSeal>,
    predecessor_debt: u64,
}
impl LifecycleIoCapacityReservation<'_> {
    /// Return the exact frozen predecessor debt only to the sealed scheduler
    /// factory capability. There is no raw production getter.
    pub(crate) const fn authenticated_predecessor_debt(
        &self,
        _factory: &super::v2_lifecycle_coordinator::AuthenticatedSchedulerInputsFactory,
    ) -> u64 {
        self.predecessor_debt
    }
    /// Move the exact Serve target into lifecycle admission while retaining
    /// the worker queue cut and reserved auxiliary slot.
    pub(in crate::sumeragi) fn take_certified_serve_target(
        &mut self,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> Result<LifecycleIngressIoTargetSeal, ()> {
        let target = self.target.as_ref().ok_or(())?;
        if target.kind() != LifecycleIngressIoTargetKind::CertifiedServe
            || !target.matches_certified_serve_request(authenticated.request_hash())
        {
            return Err(());
        }
        Ok(self
            .target
            .take()
            .expect("validated Serve reservation retains its target"))
    }

    /// Restore the unchanged Serve target returned by safe lifecycle admission.
    pub(in crate::sumeragi) fn restore_certified_serve_target(
        &mut self,
        target: LifecycleIngressIoTargetSeal,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> Result<(), LifecycleIngressIoTargetSeal> {
        if self.target.is_some()
            || target.kind() != LifecycleIngressIoTargetKind::CertifiedServe
            || !target.matches_certified_serve_request(authenticated.request_hash())
        {
            return Err(target);
        }
        self.target = Some(target);
        Ok(())
    }
    /// Reject a repeated selected Fetch while its exact work id is still
    /// queued, active, or awaiting completion acknowledgement.
    pub(crate) fn preflight_selected_target_work_absent(&self) -> bool {
        let target = self
            .target
            .as_ref()
            .expect("live reservation retains its one-shot target");
        let state = self
            .state
            .as_ref()
            .expect("live reservation retains the queue guard");
        !state
            .work
            .keys()
            .copied()
            .any(|work_id| target.matches_certified_fetch_work_id(work_id))
    }
    /// Verify that one prepared persistence command can consume this exact
    /// target slot without any fallible queue mutation after planning.
    pub(in crate::sumeragi) fn preflight_certified_fetch_body_persistence(
        &self,
        task: &CertifiedFetchBodyPersistenceTask,
    ) -> bool {
        let target = self
            .target
            .as_ref()
            .expect("live reservation retains its one-shot target");
        if target.kind() != LifecycleIngressIoTargetKind::CertifiedFetchBodyPersistence
            || !task.matches_ingress_identity(target.ingress_identity())
            || !target.matches_certified_fetch_work_id(task.work_id())
        {
            return false;
        }
        let work_id = task.work_id();
        let state = self
            .state
            .as_ref()
            .expect("live reservation retains the queue guard");
        self.preflight_selected_target_work_absent() && state.work.get(&work_id).is_none()
    }
    /// Verify that one recovered body persistence task owns this exact target
    /// and has no queued, active, or completion-pending dedicated predecessor.
    pub(in crate::sumeragi) fn preflight_recovered_decision_fetch_body_persistence(
        &self,
        task: &RecoveredDecisionFetchBodyPersistenceTaskV1,
    ) -> bool {
        let target = self
            .target
            .as_ref()
            .expect("live reservation retains its one-shot target");
        let state = self
            .state
            .as_ref()
            .expect("live reservation retains the queue guard");
        target.kind() == LifecycleIngressIoTargetKind::RecoveredDecisionFetchBodyPersistence
            && task.matches_ingress_identity(target.ingress_identity())
            && target.matches_recovered_decision_fetch_key(task.dispatch_key())
            && !state
                .recovered_decision_fetch_bodies
                .contains_key(&task.dispatch_key())
    }
    /// Reject an exact recovered target already represented anywhere in its
    /// dedicated queued/active/completion-pending index before consuming the selector.
    pub(in crate::sumeragi) fn preflight_recovered_decision_fetch_target_absent(&self) -> bool {
        let target = self
            .target
            .as_ref()
            .expect("live reservation retains its one-shot target");
        let state = self
            .state
            .as_ref()
            .expect("live reservation retains the queue guard");
        target.kind() == LifecycleIngressIoTargetKind::RecoveredDecisionFetchBodyPersistence
            && !state
                .recovered_decision_fetch_bodies
                .keys()
                .copied()
                .any(|key| target.matches_recovered_decision_fetch_key(key))
    }
    /// Verify that one claimed Serve can consume this target and owns no prior
    /// queued, active, or completion-pending worker slot.
    pub(in crate::sumeragi) fn preflight_lifecycle_certified_serve(
        &self,
        task: &LifecycleCertifiedServeTaskV1,
    ) -> bool {
        let target = self
            .target
            .as_ref()
            .expect("live reservation retains its one-shot target");
        let state = self
            .state
            .as_ref()
            .expect("live reservation retains the queue guard");
        task.authority_matches_request()
            && target.kind() == LifecycleIngressIoTargetKind::CertifiedServe
            && target.matches_certified_serve_request(task.request_hash())
            && !state
                .lifecycle_serves
                .contains_key(&task.lifecycle_ordinal())
    }
    /// Abort before planning and restore the one-shot target into its exact
    /// selector while releasing capacity under the retained queue lock.
    pub(crate) fn abort_into_prepared(
        mut self,
        mut prepared: PreparedLifecycleIngressSelector,
    ) -> PreparedLifecycleIngressSelector {
        let target = self
            .target
            .take()
            .expect("aborted reservation retains its one-shot target");
        prepared
            .restore_lifecycle_io_target(target)
            .expect("reservation target must restore only into its source selector");
        let state = self
            .state
            .take()
            .expect("aborted reservation retains the queue guard");
        self.queue.admission.release();
        drop(state);
        self.queue.ready.notify_all();
        self.operation
            .take()
            .expect("aborted reservation retains its fail-stop operation")
            .complete();
        prepared
    }
    /// Consume the locked reservation into the preflighted exact persistence
    /// command and publish the FIFO only after its ownership index is installed.
    pub(in crate::sumeragi) fn commit_certified_fetch_body_persistence(
        mut self,
        task: CertifiedFetchBodyPersistenceTask,
    ) {
        assert!(
            self.preflight_certified_fetch_body_persistence(&task),
            "reserved certified-Fetch command changed after exact preflight"
        );
        let work_id = task.work_id();
        let descriptor = V2IoWorkDescriptor::PersistCertifiedFetchBody {
            id: task.id(),
            response_hash: task.response_hash(),
        };
        let mut state = self
            .state
            .take()
            .expect("committed reservation retains the queue guard");
        let replaced = state.work.insert(
            work_id,
            V2IoTrackedWork {
                descriptor,
                state: V2IoWorkState::Queued,
            },
        );
        assert!(replaced.is_none(), "preflight forbids in-flight coalescing");
        state
            .commands
            .push_back(V2IoCommand::PersistCertifiedFetchBody(task));
        drop(state);
        self.queue.ready.notify_all();
        self.operation
            .take()
            .expect("committed reservation retains its fail-stop operation")
            .complete();
    }
    /// Publish one exact recovered persistence command under its dedicated
    /// lifecycle key. Preflight plus the retained queue mutex makes this tail
    /// assertion-only after the executor response claim is installed.
    pub(in crate::sumeragi) fn commit_recovered_decision_fetch_body_persistence(
        mut self,
        task: RecoveredDecisionFetchBodyPersistenceTaskV1,
    ) {
        assert!(
            self.preflight_recovered_decision_fetch_body_persistence(&task),
            "reserved recovered Decision Fetch persistence changed before publication"
        );
        let key = task.dispatch_key();
        let tracked = V2IoTrackedRecoveredDecisionFetchBodyV1 {
            id: task.id(),
            response_hash: task.response_hash(),
            state: V2IoWorkState::Queued,
        };
        let mut state = self
            .state
            .take()
            .expect("committed reservation retains the queue guard");
        assert!(
            state
                .recovered_decision_fetch_bodies
                .insert(key, tracked)
                .is_none(),
            "exact preflight forbids duplicate recovered body persistence"
        );
        state
            .commands
            .push_back(V2IoCommand::PersistRecoveredDecisionFetchBody(task));
        drop(state);
        self.queue.ready.notify_all();
        self.operation
            .take()
            .expect("committed recovered persistence retains its fail-stop operation")
            .complete();
    }
    /// Publish one lifecycle-owned Serve command after exact scheduler claim
    /// and physical dequeue have made every remaining mutation infallible.
    pub(in crate::sumeragi) fn commit_lifecycle_certified_serve(
        mut self,
        task: LifecycleCertifiedServeTaskV1,
    ) {
        assert!(
            self.preflight_lifecycle_certified_serve(&task),
            "reserved lifecycle Certified-Serve changed before publication"
        );
        let ordinal = task.lifecycle_ordinal();
        let tracked = V2IoTrackedLifecycleServeV1 {
            request_hash: task.request_hash(),
            state: V2IoWorkState::Queued,
        };
        let mut state = self
            .state
            .take()
            .expect("committed Serve reservation retains the queue guard");
        assert!(
            state.lifecycle_serves.insert(ordinal, tracked).is_none(),
            "exact Serve preflight forbids duplicate lifecycle dispatch"
        );
        state
            .commands
            .push_back(V2IoCommand::LifecycleCertifiedServe(task));
        drop(self.target.take());
        drop(state);
        self.queue.ready.notify_all();
        self.operation
            .take()
            .expect("committed Serve reservation retains its fail-stop operation")
            .complete();
    }

    /// Release an unchanged Serve reservation before scheduler claim or dequeue.
    pub(in crate::sumeragi) fn abort_certified_serve_before_plan(mut self) {
        let state = self
            .state
            .take()
            .expect("aborted Serve reservation retains the queue guard");
        self.queue.admission.release();
        drop(self.target.take());
        drop(state);
        self.queue.ready.notify_all();
        self.operation
            .take()
            .expect("aborted Serve reservation retains its fail-stop operation")
            .complete();
    }
    #[cfg(test)]
    fn cancel_before_plan_for_test(mut self) {
        let state = self
            .state
            .take()
            .expect("cancelled test reservation retains the queue guard");
        self.queue.admission.release();
        drop(state);
        self.queue.ready.notify_all();
        self.operation
            .take()
            .expect("cancelled test reservation retains its fail-stop operation")
            .complete();
    }
}
/// Locked Consensus capacity until a recovered Apply dispatch enters FIFO.
#[must_use = "the recovered Decision Apply reservation must commit its prepared dispatch"]
pub(in crate::sumeragi) struct RecoveredDecisionApplyCapacityReservationV1<'a> {
    queue: &'a V2IoCommandQueue,
    state: Option<std::sync::MutexGuard<'a, V2IoCommandQueueState>>,
    operation: Option<ConsensusFailStopOperation<'a>>,
    key: RecoveredDecisionApplyDispatchKeyV1,
}
impl RecoveredDecisionApplyCapacityReservationV1<'_> {
    /// Recheck that the claimed registry projection names this exact reservation.
    pub(in crate::sumeragi) fn preflight(
        &self,
        prepared: &PreparedRecoveredDecisionApplyDispatch<'_>,
    ) -> bool {
        let state = self
            .state
            .as_ref()
            .expect("live recovered Apply reservation retains its queue cut");
        prepared.dispatch_key() == self.key
            && !state.recovered_decision_applies.contains_key(&self.key)
    }
    /// Atomically arm the registry dispatch and publish its dedicated worker command.
    pub(in crate::sumeragi) fn commit(
        mut self,
        prepared: PreparedRecoveredDecisionApplyDispatch<'_>,
    ) {
        assert!(
            self.preflight(&prepared),
            "reserved recovered Decision Apply changed before queue publication"
        );
        let task = prepared.commit_for_worker();
        assert_eq!(
            task.dispatch_key(),
            self.key,
            "registry projection returned another recovered Apply dispatch"
        );
        let mut state = self
            .state
            .take()
            .expect("committed recovered Apply reservation retains its queue cut");
        // Take this after the queue guard so unwinding activates restart before
        // another producer can acquire the released FIFO mutex.
        let operation = self
            .operation
            .take()
            .expect("committed recovered Apply retains its fail-stop operation");
        let replaced = state.recovered_decision_applies.insert(
            self.key,
            V2IoTrackedRecoveredDecisionApplyV1 {
                state: V2IoWorkState::Queued,
            },
        );
        assert!(
            replaced.is_none(),
            "exact preflight forbids a duplicate dispatch"
        );
        state
            .commands
            .push_back(V2IoCommand::RecoveredDecisionApply(task));
        drop(state);
        self.queue.ready.notify_all();
        operation.complete();
    }
}
impl Drop for RecoveredDecisionApplyCapacityReservationV1<'_> {
    fn drop(&mut self) {
        drop(self.operation.take());
        if let Some(state) = self.state.take() {
            self.queue.admission.release();
            drop(state);
            self.queue.ready.notify_all();
        }
    }
}
/// Locked Consensus capacity for one exact lifecycle-owned recovered Sign.
#[must_use = "the recovered Sign reservation must commit its prepared dispatch"]
pub(in crate::sumeragi) struct RecoveredLifecycleSignCapacityReservationV1<'a> {
    queue: &'a V2IoCommandQueue,
    state: Option<std::sync::MutexGuard<'a, V2IoCommandQueueState>>,
    operation: Option<ConsensusFailStopOperation<'a>>,
    key: RecoveredLifecycleSignDispatchKeyV1,
    predecessor_debt: u64,
}
impl RecoveredLifecycleSignCapacityReservationV1<'_> {
    /// Return frozen worker predecessor debt only to scheduler authority.
    pub(in crate::sumeragi) const fn authenticated_predecessor_debt(
        &self,
        _factory: &AuthenticatedSchedulerInputsFactory,
    ) -> u64 {
        self.predecessor_debt
    }
    /// Recheck that the claimed registry projection names this reservation.
    pub(in crate::sumeragi) fn preflight(
        &self,
        prepared: &PreparedRecoveredLifecycleSignDispatch<'_>,
    ) -> bool {
        let state = self
            .state
            .as_ref()
            .expect("live recovered Sign reservation retains its queue cut");
        prepared.dispatch_key() == self.key
            && !state.recovered_lifecycle_signs.contains_key(&self.key)
    }
    /// Release an uncommitted queue cut after the caller proved no claim remains.
    pub(in crate::sumeragi) fn cancel_uncommitted(mut self) {
        self.operation
            .take()
            .expect("cancelled recovered Sign reservation retains its operation")
            .complete();
    }
    /// Atomically arm the closed carrier and publish its dedicated command.
    pub(in crate::sumeragi) fn commit(self, prepared: PreparedRecoveredLifecycleSignDispatch<'_>) {
        assert!(
            self.preflight(&prepared),
            "reserved recovered Sign changed before queue publication"
        );
        let task = prepared.commit_for_worker();
        self.publish_task(task);
    }
    /// Publish one exact test task through the production reservation state machine.
    #[cfg(test)]
    fn commit_for_test(self, task: RecoveredLifecycleSignTaskV1) {
        self.publish_task(task);
    }
    fn publish_task(mut self, task: RecoveredLifecycleSignTaskV1) {
        assert_eq!(
            task.dispatch_key(),
            self.key,
            "registry projection returned another recovered Sign dispatch"
        );
        let mut state = self
            .state
            .take()
            .expect("committed recovered Sign reservation retains its queue cut");
        // Take this after the queue guard so unwind ordering remains fail-stop.
        let operation = self
            .operation
            .take()
            .expect("committed recovered Sign retains its fail-stop operation");
        let replaced = state.recovered_lifecycle_signs.insert(
            self.key,
            V2IoTrackedRecoveredLifecycleSignV1 {
                state: V2IoWorkState::Queued,
            },
        );
        assert!(
            replaced.is_none(),
            "exact preflight forbids duplicate recovered Sign dispatch"
        );
        state
            .commands
            .push_back(V2IoCommand::RecoveredLifecycleSign(task));
        drop(state);
        self.queue.ready.notify_all();
        operation.complete();
    }
}
impl Drop for RecoveredLifecycleSignCapacityReservationV1<'_> {
    fn drop(&mut self) {
        drop(self.operation.take());
        if let Some(state) = self.state.take() {
            self.queue.admission.release();
            drop(state);
            self.queue.ready.notify_all();
        }
    }
}
/// Typed capacity result for the dedicated recovered Sign queue.
#[must_use = "the recovered Sign capacity result must be consumed"]
pub(in crate::sumeragi) enum RecoveredLifecycleSignCapacityCaptureV1<'a> {
    /// Exact queue position is reserved for registry projection commit.
    Reserved(RecoveredLifecycleSignCapacityReservationV1<'a>),
    /// No Consensus position was available; no logical row was claimed.
    Unavailable,
}
/// Failure before recovered Sign worker capacity can be retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum RecoveredLifecycleSignCapacityCaptureErrorV1 {
    /// The dispatch key belongs to another immutable height context.
    ForeignContext,
    /// The height-local worker command corridor is closed.
    Disconnected,
    /// Canonical output admission already requires restart.
    OutputClosed,
    /// The exact queue position is not representable in scheduler rank.
    PositionOverflow,
    /// The same carrier already owns queued, active, or pending work.
    AlreadyDispatched,
}
/// One registry-authenticated recovered Completion row whose physical corridor
/// must participate in the same scheduler snapshot as every peer row.
#[must_use = "every recovered Completion probe must enter one composite census"]
pub(in crate::sumeragi) enum RecoveredCompletionCapacityProbeV1 {
    /// One recovered Decision Apply bound to its dedicated worker key.
    Apply {
        /// Exact logical Ready ordinal.
        ordinal: u128,
        /// Closed worker dispatch key retained by the registry attestation.
        key: RecoveredDecisionApplyDispatchKeyV1,
    },
    /// One recovered lifecycle Sign bound to its dedicated worker key.
    Sign {
        /// Exact logical Ready ordinal.
        ordinal: u128,
        /// Closed worker dispatch key retained by the registry attestation.
        key: RecoveredLifecycleSignDispatchKeyV1,
    },
    /// One recovered Decision Fetch bound to its signed request owner.
    Fetch {
        /// Exact logical Ready ordinal.
        ordinal: u128,
        /// Service-authenticated request owner retained until one row is selected.
        owner: RecoveredDecisionFetchRequestOwnerV1,
        /// Exact executor-catalog capacity observed before the service locks.
        executor_available: bool,
    },
}

enum RecoveredCompletionPreparedCapacityV1 {
    Apply {
        key: RecoveredDecisionApplyDispatchKeyV1,
        available: bool,
    },
    Sign {
        key: RecoveredLifecycleSignDispatchKeyV1,
        available: bool,
    },
    Fetch {
        owner: RecoveredDecisionFetchRequestOwnerV1,
        fanout: Option<PendingExactFanout>,
        available: bool,
    },
}

impl RecoveredCompletionPreparedCapacityV1 {
    const fn available(&self) -> bool {
        match self {
            Self::Apply { available, .. }
            | Self::Sign { available, .. }
            | Self::Fetch { available, .. } => *available,
        }
    }

    const fn predecessor_debt(&self, worker_debt: u64, output_debt: u64) -> u64 {
        match self {
            Self::Apply { .. } | Self::Sign { .. } => worker_debt,
            Self::Fetch { .. } => output_debt,
        }
    }
}

/// Fail-stop snapshot freezing both recovered-Completion corridors; armed Drop
/// closes output before releasing either mutex.
#[must_use = "the recovered Completion census must select one row or complete unchanged"]
pub(in crate::sumeragi) struct RecoveredCompletionCapacityCensusV1<'service> {
    operation: Option<ConsensusFailStopOperation<'service>>,
    pending: Option<std::sync::MutexGuard<'service, PendingExactOutput>>,
    queue: &'service V2IoCommandQueue,
    state: Option<std::sync::MutexGuard<'service, V2IoCommandQueueState>>,
    worker_predecessor_debt: u64,
    output_predecessor_debt: u64,
    candidates: BTreeMap<u128, RecoveredCompletionPreparedCapacityV1>,
}

impl<'service> RecoveredCompletionCapacityCensusV1<'service> {
    /// Return one row's frozen physical availability and predecessor debt.
    pub(in crate::sumeragi) fn authenticated_capacity(
        &self,
        ordinal: u128,
        _factory: &AuthenticatedSchedulerInputsFactory,
    ) -> Option<(bool, u64)> {
        self.candidates.get(&ordinal).map(|candidate| {
            (
                candidate.available(),
                candidate
                    .predecessor_debt(self.worker_predecessor_debt, self.output_predecessor_debt),
            )
        })
    }

    /// Inspect one frozen row without minting a production scheduler factory.
    #[cfg(test)]
    fn capacity_for_test(&self, ordinal: u128) -> Option<(bool, u64)> {
        self.candidates.get(&ordinal).map(|candidate| {
            (
                candidate.available(),
                candidate
                    .predecessor_debt(self.worker_predecessor_debt, self.output_predecessor_debt),
            )
        })
    }

    /// Release an unchanged composite snapshot when no physical row is selectable.
    pub(in crate::sumeragi) fn complete_without_selection(mut self) {
        drop(self.state.take());
        drop(self.pending.take());
        self.operation
            .take()
            .expect("recovered Completion census retains its fail-stop operation")
            .complete();
    }

    /// Transfer the selected Apply row into its existing typed worker reservation.
    pub(in crate::sumeragi) fn select_apply(
        mut self,
        ordinal: u128,
    ) -> Result<RecoveredDecisionApplyCapacityReservationV1<'service>, Self> {
        let Some(RecoveredCompletionPreparedCapacityV1::Apply {
            key,
            available: true,
        }) = self.candidates.remove(&ordinal)
        else {
            return Err(self);
        };
        let state = self
            .state
            .take()
            .expect("selected recovered Apply retains the worker queue cut");
        let operation = self
            .operation
            .take()
            .expect("selected recovered Apply retains the fail-stop operation");
        assert!(
            state.commands.len() < self.queue.capacity
                && self
                    .queue
                    .admission
                    .try_reserve(V2IoAdmissionClass::Consensus),
            "frozen recovered Apply capacity changed before selection"
        );
        drop(self.pending.take());
        Ok(RecoveredDecisionApplyCapacityReservationV1 {
            queue: self.queue,
            state: Some(state),
            operation: Some(operation),
            key,
        })
    }

    /// Transfer the selected Sign row into its existing typed worker reservation.
    pub(in crate::sumeragi) fn select_sign(
        mut self,
        ordinal: u128,
    ) -> Result<RecoveredLifecycleSignCapacityReservationV1<'service>, Self> {
        let Some(RecoveredCompletionPreparedCapacityV1::Sign {
            key,
            available: true,
        }) = self.candidates.remove(&ordinal)
        else {
            return Err(self);
        };
        let state = self
            .state
            .take()
            .expect("selected recovered Sign retains the worker queue cut");
        let operation = self
            .operation
            .take()
            .expect("selected recovered Sign retains the fail-stop operation");
        assert!(
            state.commands.len() < self.queue.capacity
                && self
                    .queue
                    .admission
                    .try_reserve(V2IoAdmissionClass::Consensus),
            "frozen recovered Sign capacity changed before selection"
        );
        drop(self.pending.take());
        Ok(RecoveredLifecycleSignCapacityReservationV1 {
            queue: self.queue,
            state: Some(state),
            operation: Some(operation),
            key,
            predecessor_debt: self.worker_predecessor_debt,
        })
    }

    /// Transfer the selected Fetch row into its request owner and output reservation.
    pub(in crate::sumeragi) fn select_fetch(
        mut self,
        ordinal: u128,
    ) -> Result<
        (
            RecoveredDecisionFetchRequestOwnerV1,
            RecoveredDecisionFetchExactOutputReservationV1<'service>,
        ),
        Self,
    > {
        let Some(RecoveredCompletionPreparedCapacityV1::Fetch {
            owner,
            fanout,
            available: true,
        }) = self.candidates.remove(&ordinal)
        else {
            return Err(self);
        };
        drop(self.state.take());
        let pending = self
            .pending
            .take()
            .expect("selected recovered Fetch retains the exact-output cut");
        let operation = self
            .operation
            .take()
            .expect("selected recovered Fetch retains the fail-stop operation");
        Ok((
            owner,
            RecoveredDecisionFetchExactOutputReservationV1 {
                operation: Some(operation),
                pending: Some(pending),
                fanout,
            },
        ))
    }
}

impl Drop for RecoveredCompletionCapacityCensusV1<'_> {
    fn drop(&mut self) {
        // Activate restart while both corridors are still frozen. The locks
        // are released only after this custom Drop returns.
        drop(self.operation.take());
    }
}

impl Drop for LifecycleIoCapacityReservation<'_> {
    fn drop(&mut self) {
        // An incomplete reservation is fatal. Close output admission while the
        // queue cut is still frozen so no new operation can cross a gap before
        // rollback releases the reserved slot.
        drop(self.operation.take());
        if let Some(state) = self.state.take() {
            self.queue.admission.release();
            drop(state);
            self.queue.ready.notify_all();
        }
    }
}
enum V2IoLifecycleCapacityCapture<'a> {
    Reserved(LifecycleIoCapacityReservation<'a>),
    Unavailable(LifecycleIoCapacityWait),
}
/// Failure before the I/O service can issue a target-capacity result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LifecycleIoCapacityCaptureFailure {
    /// The prepared selector has no supported I/O target carrier.
    InvalidTarget,
    /// The selector belongs to another immutable height context.
    ForeignContext,
    /// Canonical output admission has already closed for restart.
    OutputClosed,
    /// The height-local I/O worker is absent or disconnected.
    Disconnected,
    /// The exact queue position is not representable in the scheduler rank.
    PositionOverflow,
    /// The service-owned release generation reached its terminal value.
    GenerationExhausted,
}
/// Opaque atomic result retaining both the selector and its service authority.
///
/// Neither the reservation nor an unavailable wait can be separated from the
/// selector that minted its one-shot target seal.
#[must_use = "the prepared capacity transaction must enter owner planning or be dropped"]
pub(crate) struct LifecycleIoCapacityCapture<'a> {
    outcome: LifecycleIoCapacityOutcome<'a>,
}
enum LifecycleIoCapacityOutcome<'a> {
    Reserved {
        reservation: LifecycleIoCapacityReservation<'a>,
        prepared: PreparedLifecycleIngressSelector,
    },
    Unavailable {
        wait: LifecycleIoCapacityWait,
        prepared: PreparedLifecycleIngressSelector,
    },
}
/// Exact auxiliary-capacity result for the lifecycle-owned Serve path.
#[must_use = "the Certified-Serve capacity result must be consumed"]
pub(in crate::sumeragi) enum LifecycleCertifiedServeCapacityCaptureV1<'a> {
    /// The worker FIFO and auxiliary admission slot remain locked.
    Reserved(LifecycleIoCapacityReservation<'a>),
    /// No slot was available; retry is fenced by the exact release generation.
    Unavailable(LifecycleCertifiedServeCapacityWaitV1),
}
/// Opaque release-generation wait for one lifecycle-owned Serve request.
#[must_use = "the retained Serve must not reprobe before this wait advances"]
pub(in crate::sumeragi) struct LifecycleCertifiedServeCapacityWaitV1 {
    wait: LifecycleIoCapacityWait,
}
impl LifecycleCertifiedServeCapacityWaitV1 {
    /// Classify whether the exact worker released capacity since capture.
    pub(in crate::sumeragi) fn status(
        &self,
        services: &ProductionV2Services,
    ) -> LifecycleIoCapacityWaitStatus {
        self.wait.status(services)
    }
}
/// Capacity authority opened only by the sealed scheduler factory permit.
pub(crate) enum AuthenticatedLifecycleIoCapacity<'a> {
    /// A live locked slot retains the complete selected carrier.
    Reserved {
        /// Exact service reservation.
        reservation: LifecycleIoCapacityReservation<'a>,
        /// Complete selector which minted the reservation target.
        prepared: PreparedLifecycleIngressSelector,
    },
    /// No slot was available at the retained release generation.
    Unavailable {
        /// Opaque service generation wait.
        wait: LifecycleIoCapacityWait,
        /// Complete selector retained with that wait.
        prepared: PreparedLifecycleIngressSelector,
    },
}
impl<'a> LifecycleIoCapacityCapture<'a> {
    /// Open this transaction only for the non-clone sealed scheduler factory.
    pub(crate) fn into_authenticated(
        self,
        _factory: &AuthenticatedSchedulerInputsFactory,
    ) -> AuthenticatedLifecycleIoCapacity<'a> {
        match self.outcome {
            LifecycleIoCapacityOutcome::Reserved {
                reservation,
                prepared,
            } => AuthenticatedLifecycleIoCapacity::Reserved {
                reservation,
                prepared,
            },
            LifecycleIoCapacityOutcome::Unavailable { wait, prepared } => {
                AuthenticatedLifecycleIoCapacity::Unavailable { wait, prepared }
            }
        }
    }
}
/// Ownership-preserving failure before any capacity authority was acquired.
#[must_use = "the complete selector remains available for a corrected attempt"]
pub(crate) struct LifecycleIoCapacityCaptureError {
    failure: LifecycleIoCapacityCaptureFailure,
    prepared: PreparedLifecycleIngressSelector,
}
impl LifecycleIoCapacityCaptureError {
    /// Return the closed service failure classification.
    pub(crate) const fn failure(&self) -> LifecycleIoCapacityCaptureFailure {
        self.failure
    }
    /// Recover the complete selector with its one-shot target restored.
    pub(crate) fn into_prepared(self) -> PreparedLifecycleIngressSelector {
        self.prepared
    }
}
#[allow(
    dead_code,
    reason = "non-full failures retain the rejected command for ownership symmetry and diagnostics"
)]
enum V2IoTrySendError {
    Full(V2IoCommand),
    Disconnected(V2IoCommand),
    ConflictingWorkId {
        work_id: EffectWorkId,
        command: V2IoCommand,
    },
    UnreservedRecoveredDecisionApply {
        key: RecoveredDecisionApplyDispatchKeyV1,
        command: V2IoCommand,
    },
}
impl std::fmt::Debug for V2IoTrySendError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Full(_) => formatter.write_str("Full(..)"),
            Self::Disconnected(_) => formatter.write_str("Disconnected(..)"),
            Self::ConflictingWorkId { work_id, .. } => formatter
                .debug_struct("ConflictingWorkId")
                .field("work_id", work_id)
                .finish(),
            Self::UnreservedRecoveredDecisionApply { key, .. } => formatter
                .debug_struct("UnreservedRecoveredDecisionApply")
                .field("key", key)
                .finish(),
        }
    }
}
#[cfg(test)]
fn v2_io_command_channel(
    capacity: usize,
    _roster_serve_capacity: usize,
    _observer_source_capacity: usize,
    _observer_per_source_capacity: usize,
    admission: Arc<V2IoAdmission>,
) -> (V2IoCommandSender, V2IoCommandReceiver) {
    build_v2_io_command_channel(capacity, admission)
}
pub(super) fn certified_serve_family_capacity(
    roster_serve_capacity: usize,
    observer_source_capacity: usize,
    observer_per_source_capacity: usize,
) -> Result<usize, String> {
    assert!(
        roster_serve_capacity != 0
            || (observer_source_capacity != 0 && observer_per_source_capacity != 0),
        "Sumeragi v2 Serve owner capacity must be non-zero"
    );
    roster_serve_capacity
        .checked_add(
            observer_source_capacity
                .checked_mul(observer_per_source_capacity)
                .ok_or_else(|| {
                    "bounded observer Serve owner capacity must not overflow".to_owned()
                })?,
        )
        .and_then(|owners| owners.checked_mul(CERTIFIED_SERVE_PHASE_FAMILIES))
        .ok_or_else(|| "bounded Serve phase-family capacity must not overflow".to_owned())
}
fn build_v2_io_command_channel(
    capacity: usize,
    admission: Arc<V2IoAdmission>,
) -> (V2IoCommandSender, V2IoCommandReceiver) {
    let queue = Arc::new(V2IoCommandQueue {
        capacity,
        admission,
        state: Mutex::new(V2IoCommandQueueState {
            commands: VecDeque::with_capacity(capacity.min(1_024)),
            work: BTreeMap::new(),
            recovered_decision_applies: BTreeMap::new(),
            recovered_lifecycle_signs: BTreeMap::new(),
            recovered_decision_fetch_bodies: BTreeMap::new(),
            lifecycle_serves: BTreeMap::new(),
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
    fn capture_lifecycle_capacity<'a>(
        self: &'a Arc<Self>,
        operation: ConsensusFailStopOperation<'a>,
        output_guard: Arc<ConsensusOutputGuard>,
        target: LifecycleIngressIoTargetSeal,
    ) -> Result<
        V2IoLifecycleCapacityCapture<'a>,
        (
            LifecycleIoCapacityCaptureFailure,
            LifecycleIngressIoTargetSeal,
        ),
    > {
        let class = match target.kind() {
            LifecycleIngressIoTargetKind::CertifiedServe => V2IoAdmissionClass::Auxiliary,
            LifecycleIngressIoTargetKind::CertifiedFetchBodyPersistence => {
                V2IoAdmissionClass::Consensus
            }
            LifecycleIngressIoTargetKind::RecoveredDecisionFetchBodyPersistence => {
                V2IoAdmissionClass::Consensus
            }
        };
        let state = self.lock();
        if !state.sender_open || !state.receiver_open {
            drop(operation);
            return Err((LifecycleIoCapacityCaptureFailure::Disconnected, target));
        }
        if self.admission.lifecycle_capacity_generation_exhausted() {
            drop(operation);
            return Err((
                LifecycleIoCapacityCaptureFailure::GenerationExhausted,
                target,
            ));
        }
        let predecessor_debt = match u64::try_from(state.commands.len()) {
            Ok(debt) => debt,
            Err(_) => {
                drop(operation);
                return Err((LifecycleIoCapacityCaptureFailure::PositionOverflow, target));
            }
        };
        if state.commands.len() >= self.capacity || !self.admission.try_reserve(class) {
            let observed_generation = self.admission.lifecycle_capacity_generation();
            drop(state);
            operation.complete();
            return Ok(V2IoLifecycleCapacityCapture::Unavailable(
                LifecycleIoCapacityWait {
                    queue: Arc::clone(self),
                    output_guard,
                    target,
                    observed_generation,
                },
            ));
        }
        Ok(V2IoLifecycleCapacityCapture::Reserved(
            LifecycleIoCapacityReservation {
                queue: self.as_ref(),
                state: Some(state),
                operation: Some(operation),
                target: Some(target),
                predecessor_debt,
            },
        ))
    }
    fn capture_recovered_lifecycle_sign_capacity<'a>(
        self: &'a Arc<Self>,
        operation: ConsensusFailStopOperation<'a>,
        key: RecoveredLifecycleSignDispatchKeyV1,
    ) -> Result<
        RecoveredLifecycleSignCapacityCaptureV1<'a>,
        RecoveredLifecycleSignCapacityCaptureErrorV1,
    > {
        let state = self.lock();
        if !state.sender_open || !state.receiver_open {
            operation.complete();
            return Err(RecoveredLifecycleSignCapacityCaptureErrorV1::Disconnected);
        }
        if state.recovered_lifecycle_signs.contains_key(&key) {
            operation.complete();
            return Err(RecoveredLifecycleSignCapacityCaptureErrorV1::AlreadyDispatched);
        }
        let predecessor_debt = match u64::try_from(state.commands.len()) {
            Ok(debt) => debt,
            Err(_) => {
                operation.complete();
                return Err(RecoveredLifecycleSignCapacityCaptureErrorV1::PositionOverflow);
            }
        };
        if state.commands.len() >= self.capacity
            || !self.admission.try_reserve(V2IoAdmissionClass::Consensus)
        {
            operation.complete();
            return Ok(RecoveredLifecycleSignCapacityCaptureV1::Unavailable);
        }
        Ok(RecoveredLifecycleSignCapacityCaptureV1::Reserved(
            RecoveredLifecycleSignCapacityReservationV1 {
                queue: self.as_ref(),
                state: Some(state),
                operation: Some(operation),
                key,
                predecessor_debt,
            },
        ))
    }
    /// Project worker capacity for one recovered candidate without changing the queue cut.
    fn recovered_completion_worker_capacity(&self, state: &V2IoCommandQueueState) -> bool {
        state.commands.len() < self.capacity
            && self.admission.queued() < self.admission.limit(V2IoAdmissionClass::Consensus)
    }

    fn try_send_as(
        &self,
        class: V2IoAdmissionClass,
        command: V2IoCommand,
    ) -> Result<(), V2IoTrySendError> {
        if let Some(key) = command.recovered_decision_apply_key() {
            return Err(V2IoTrySendError::UnreservedRecoveredDecisionApply { key, command });
        }
        assert!(
            command.recovered_lifecycle_sign_key().is_none(),
            "recovered Sign commands require their locked lifecycle reservation"
        );
        assert!(
            command.recovered_decision_fetch_key().is_none(),
            "recovered Decision Fetch persistence requires its locked lifecycle reservation"
        );
        assert!(
            command.lifecycle_certified_serve_ordinal().is_none(),
            "lifecycle Certified-Serve commands require their locked auxiliary reservation"
        );
        let descriptor = command.work_descriptor();
        let mut state = self.lock();
        if !state.sender_open || !state.receiver_open {
            return Err(V2IoTrySendError::Disconnected(command));
        }
        if let Some((work_id, descriptor)) = &descriptor
            && let Some(existing) = state.work.get(work_id)
        {
            if existing.descriptor == *descriptor {
                return Ok(());
            }
            return Err(V2IoTrySendError::ConflictingWorkId {
                work_id: *work_id,
                command,
            });
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
            return Err(format!(
                "Sumeragi v2 I/O work {} has no tracked owner",
                work_id.get()
            ));
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
        drop(state);
        self.ready.notify_all();
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
                if let Some(key) = command.recovered_decision_apply_key() {
                    let tracked = state
                        .recovered_decision_applies
                        .get_mut(&key)
                        .expect("queued recovered Decision Apply must have an ownership record");
                    assert_eq!(tracked.state, V2IoWorkState::Queued);
                    tracked.state = V2IoWorkState::Active;
                }
                if let Some(key) = command.recovered_lifecycle_sign_key() {
                    let tracked = state
                        .recovered_lifecycle_signs
                        .get_mut(&key)
                        .expect("queued recovered Sign must have an ownership record");
                    assert_eq!(tracked.state, V2IoWorkState::Queued);
                    tracked.state = V2IoWorkState::Active;
                }
                if let Some(key) = command.recovered_decision_fetch_key() {
                    let tracked = state
                        .recovered_decision_fetch_bodies
                        .get_mut(&key)
                        .expect("queued recovered Decision Fetch body must retain its owner");
                    assert_eq!(tracked.state, V2IoWorkState::Queued);
                    tracked.state = V2IoWorkState::Active;
                }
                if let Some(ordinal) = command.lifecycle_certified_serve_ordinal() {
                    let tracked = state
                        .lifecycle_serves
                        .get_mut(&ordinal)
                        .expect("queued lifecycle Certified-Serve must retain its exact owner");
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
        if let Some(key) = command.recovered_decision_apply_key() {
            let tracked = state
                .recovered_decision_applies
                .get_mut(&key)
                .expect("queued recovered Decision Apply must have an ownership record");
            assert_eq!(tracked.state, V2IoWorkState::Queued);
            tracked.state = V2IoWorkState::Active;
        }
        if let Some(key) = command.recovered_lifecycle_sign_key() {
            let tracked = state
                .recovered_lifecycle_signs
                .get_mut(&key)
                .expect("queued recovered Sign must have an ownership record");
            assert_eq!(tracked.state, V2IoWorkState::Queued);
            tracked.state = V2IoWorkState::Active;
        }
        if let Some(key) = command.recovered_decision_fetch_key() {
            let tracked = state
                .recovered_decision_fetch_bodies
                .get_mut(&key)
                .expect("queued recovered Decision Fetch body must retain its owner");
            assert_eq!(tracked.state, V2IoWorkState::Queued);
            tracked.state = V2IoWorkState::Active;
        }
        if let Some(ordinal) = command.lifecycle_certified_serve_ordinal() {
            let tracked = state
                .lifecycle_serves
                .get_mut(&ordinal)
                .expect("queued lifecycle Certified-Serve must retain its exact owner");
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
    fn complete_recovered_decision_apply(
        &self,
        key: RecoveredDecisionApplyDispatchKeyV1,
        result: &RecoveredDecisionApplyWorkerResultV1,
    ) -> Result<(), String> {
        let mut state = self.lock();
        let tracked = state
            .recovered_decision_applies
            .get_mut(&key)
            .ok_or_else(|| {
                "completed recovered Decision Apply lost its lifecycle owner".to_owned()
            })?;
        if tracked.state != V2IoWorkState::Active || result.dispatch_key() != key {
            return Err(
                "completed recovered Decision Apply changed its exact dispatch material".to_owned(),
            );
        }
        tracked.state = V2IoWorkState::CompletionPending;
        Ok(())
    }
    fn complete_recovered_lifecycle_sign(
        &self,
        key: RecoveredLifecycleSignDispatchKeyV1,
        result: &RecoveredLifecycleSignWorkerResultV1,
    ) -> Result<(), String> {
        let mut state = self.lock();
        let tracked = state
            .recovered_lifecycle_signs
            .get_mut(&key)
            .ok_or_else(|| "completed recovered Sign lost its lifecycle owner".to_owned())?;
        if tracked.state != V2IoWorkState::Active
            || result.dispatch_key() != key
            || !result.is_exact()
        {
            return Err("completed recovered Sign changed its exact dispatch material".to_owned());
        }
        tracked.state = V2IoWorkState::CompletionPending;
        Ok(())
    }
    fn complete_recovered_decision_fetch_body(
        &self,
        key: RecoveredDecisionFetchDispatchKeyV1,
        completion: &RecoveredDecisionFetchBodyPersistenceCompletionV1,
    ) -> Result<(), String> {
        let mut state = self.lock();
        let tracked = state
            .recovered_decision_fetch_bodies
            .get_mut(&key)
            .ok_or_else(|| {
                "completed recovered Decision Fetch body lost its lifecycle owner".to_owned()
            })?;
        if tracked.state != V2IoWorkState::Active
            || completion.dispatch_key() != key
            || completion.id() != tracked.id
            || completion.response_hash() != tracked.response_hash
        {
            return Err(
                "completed recovered Decision Fetch body changed its exact persistence material"
                    .to_owned(),
            );
        }
        tracked.state = V2IoWorkState::CompletionPending;
        Ok(())
    }
    fn complete_lifecycle_certified_serve(
        &self,
        result: &LifecycleCertifiedServeWorkerResultV1,
    ) -> Result<(), String> {
        let ordinal = result.lifecycle_ordinal();
        let mut state = self.lock();
        let tracked = state.lifecycle_serves.get_mut(&ordinal).ok_or_else(|| {
            "completed lifecycle Certified-Serve lost its exact queue owner".to_owned()
        })?;
        if tracked.state != V2IoWorkState::Active
            || tracked.request_hash != result.request_hash()
            || result.response.request_hash != tracked.request_hash
        {
            return Err(
                "completed lifecycle Certified-Serve changed its lease or request".to_owned(),
            );
        }
        tracked.state = V2IoWorkState::CompletionPending;
        Ok(())
    }
    fn retry_recovered_decision_apply<T: RecoveredDecisionApplyRetryTaskV1>(
        &self,
        task: T,
    ) -> Result<(), RecoveredDecisionApplyRetryQueueErrorV1<T>> {
        let key = task.dispatch_key();
        let mut state = self.lock();
        if !state.sender_open
            || !state.receiver_open
            || !self
                .admission
                .recovered_decision_apply_completion_is_exact(key)
            || state
                .recovered_decision_applies
                .get(&key)
                .is_none_or(|tracked| tracked.state != V2IoWorkState::CompletionPending)
            || state
                .commands
                .iter()
                .any(|command| command.recovered_decision_apply_key() == Some(key))
        {
            return Err(RecoveredDecisionApplyRetryQueueErrorV1::InvalidOwner(task));
        }
        if state.commands.len() >= self.capacity
            || !self.admission.try_reserve(V2IoAdmissionClass::Consensus)
        {
            return Err(RecoveredDecisionApplyRetryQueueErrorV1::Unavailable(task));
        }
        // Transfer the exact keyed completion slot back to the command FIFO
        // while the same queue cut is still locked. The worker cannot publish
        // the replacement completion before the old owner is gone.
        assert!(
            self.admission
                .transfer_recovered_decision_apply_completion(key),
            "locked recovered Apply retry must retain its exact completion owner"
        );
        state
            .recovered_decision_applies
            .get_mut(&key)
            .expect("validated recovered Apply retry retains its command owner")
            .state = V2IoWorkState::Queued;
        state.commands.push_back(task.into_command());
        drop(state);
        self.ready.notify_all();
        Ok(())
    }
    fn acknowledge_completion(&self, work_id: EffectWorkId) {
        let mut state = self.lock();
        let tracked = state
            .work
            .remove(&work_id)
            .expect("delivered Sumeragi v2 completion must have an ownership record");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
    }
    fn prepare_recovered_decision_apply_ack(
        self: &Arc<Self>,
        key: RecoveredDecisionApplyDispatchKeyV1,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<RecoveredDecisionApplyWorkAckV1, String> {
        let state = self.lock();
        let tracked = state.recovered_decision_applies.get(&key).ok_or_else(|| {
            "recovered Decision Apply completion lost its exact command owner".to_owned()
        })?;
        if tracked.state != V2IoWorkState::CompletionPending {
            return Err(
                "recovered Decision Apply completion crossed a non-pending command owner"
                    .to_owned(),
            );
        }
        if !self
            .admission
            .recovered_decision_apply_completion_is_exact(key)
        {
            return Err(
                "recovered Decision Apply completion changed its bounded FIFO ownership".to_owned(),
            );
        }
        drop(state);
        Ok(RecoveredDecisionApplyWorkAckV1 {
            queue: Arc::clone(self),
            output_guard,
            key,
            armed: true,
        })
    }
    fn transfer_recovered_lifecycle_sign_completion(
        self: &Arc<Self>,
        key: RecoveredLifecycleSignDispatchKeyV1,
        ownership_position: usize,
    ) -> bool {
        let state = self.lock();
        let pending = state
            .recovered_lifecycle_signs
            .get(&key)
            .is_some_and(|tracked| tracked.state == V2IoWorkState::CompletionPending);
        if !pending {
            return false;
        }
        self.admission
            .transfer_recovered_lifecycle_sign_completion_at(key, ownership_position)
    }
    fn acknowledge_recovered_decision_apply(&self, key: RecoveredDecisionApplyDispatchKeyV1) {
        let mut state = self.lock();
        let tracked = state
            .recovered_decision_applies
            .remove(&key)
            .expect("settled recovered Decision Apply must retain its exact command owner");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
    }
    fn acknowledge_recovered_lifecycle_sign(&self, key: RecoveredLifecycleSignDispatchKeyV1) {
        let mut state = self.lock();
        let tracked = state
            .recovered_lifecycle_signs
            .remove(&key)
            .expect("settled recovered Sign must retain its exact command owner");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
    }
    fn acknowledge_recovered_decision_fetch_body(
        &self,
        key: RecoveredDecisionFetchDispatchKeyV1,
        id: super::v2_lifecycle_coordinator::RecoveredDecisionFetchBodyPersistenceIdV1,
        response_hash: HashOf<wire::CertifiedBodyResponse>,
    ) {
        let mut state = self.lock();
        let tracked = state
            .recovered_decision_fetch_bodies
            .remove(&key)
            .expect("settled recovered Decision Fetch must retain its exact command owner");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
        assert_eq!(tracked.id, id);
        assert_eq!(tracked.response_hash, response_hash);
    }
    fn transfer_lifecycle_certified_serve_completion(
        self: &Arc<Self>,
        ordinal: u128,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
        ownership_position: usize,
    ) -> bool {
        let state = self.lock();
        let pending = state.lifecycle_serves.get(&ordinal).is_some_and(|tracked| {
            tracked.state == V2IoWorkState::CompletionPending
                && tracked.request_hash == request_hash
        });
        drop(state);
        pending
            && self
                .admission
                .transfer_lifecycle_certified_serve_completion_at(ordinal, ownership_position)
    }
    fn acknowledge_lifecycle_certified_serve(
        &self,
        ordinal: u128,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
    ) {
        let mut state = self.lock();
        let tracked = state
            .lifecycle_serves
            .remove(&ordinal)
            .expect("settled lifecycle Certified-Serve retains its exact queue owner");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
        assert_eq!(tracked.request_hash, request_hash);
    }
    fn prepare_certified_fetch_body_persistence_ack(
        self: &Arc<Self>,
        completion: &CertifiedFetchBodyPersistenceCompletion,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<CertifiedFetchBodyPersistenceWorkAck, String> {
        let work_id = completion.work_id();
        let descriptor = V2IoWorkDescriptor::PersistCertifiedFetchBody {
            id: completion.id(),
            response_hash: completion.response_hash(),
        };
        let state = self.lock();
        let tracked = state.work.get(&work_id).ok_or_else(|| {
            format!(
                "persisted certified-Fetch body work {} lost its exact command owner",
                work_id.get()
            )
        })?;
        if tracked.state != V2IoWorkState::CompletionPending || tracked.descriptor != descriptor {
            return Err(format!(
                "persisted certified-Fetch body work {} changed its exact command owner",
                work_id.get()
            ));
        }
        drop(state);
        Ok(CertifiedFetchBodyPersistenceWorkAck {
            queue: Arc::clone(self),
            output_guard,
            work_id,
            descriptor,
            armed: true,
        })
    }
    fn acknowledge_exact_lifecycle_completion(
        &self,
        work_id: EffectWorkId,
        descriptor: &V2IoWorkDescriptor,
    ) {
        let mut state = self.lock();
        let tracked = state
            .work
            .get(&work_id)
            .expect("preflighted lifecycle completion retains its exact command owner");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
        assert_eq!(&tracked.descriptor, descriptor);
        state
            .work
            .remove(&work_id)
            .expect("preflighted lifecycle completion work remains indexed");
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
        assert!(
            state
                .lifecycle_serves
                .values()
                .all(|tracked| tracked.state == V2IoWorkState::CompletionPending),
            "receiver teardown cannot abandon a queued or active lifecycle Certified-Serve"
        );
        state.commands.clear();
        // A normal Shutdown/Retire exit can close the command receiver while
        // already-sent completions remain buffered. Keep those ownership
        // records until the serialized handle drains and acknowledges them.
        state
            .work
            .retain(|_, tracked| tracked.state == V2IoWorkState::CompletionPending);
        state
            .recovered_decision_applies
            .retain(|_, tracked| tracked.state == V2IoWorkState::CompletionPending);
        state
            .recovered_lifecycle_signs
            .retain(|_, tracked| tracked.state == V2IoWorkState::CompletionPending);
        state
            .recovered_decision_fetch_bodies
            .retain(|_, tracked| tracked.state == V2IoWorkState::CompletionPending);
        state
            .lifecycle_serves
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
    fn complete_recovered_decision_apply(
        &self,
        key: RecoveredDecisionApplyDispatchKeyV1,
        result: &RecoveredDecisionApplyWorkerResultV1,
    ) -> Result<(), String> {
        self.queue.complete_recovered_decision_apply(key, result)
    }
    fn complete_recovered_lifecycle_sign(
        &self,
        key: RecoveredLifecycleSignDispatchKeyV1,
        result: &RecoveredLifecycleSignWorkerResultV1,
    ) -> Result<(), String> {
        self.queue.complete_recovered_lifecycle_sign(key, result)
    }
    fn complete_recovered_decision_fetch_body(
        &self,
        key: RecoveredDecisionFetchDispatchKeyV1,
        completion: &RecoveredDecisionFetchBodyPersistenceCompletionV1,
    ) -> Result<(), String> {
        self.queue
            .complete_recovered_decision_fetch_body(key, completion)
    }
    fn complete_lifecycle_certified_serve(
        &self,
        result: &LifecycleCertifiedServeWorkerResultV1,
    ) -> Result<(), String> {
        self.queue.complete_lifecycle_certified_serve(result)
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
/// Persisted certified-Fetch completion guarded fail-stop until typed drain
/// validates its work index and prepares the exact acknowledgement.
struct CertifiedFetchBodyPersistenceDropGuard {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl CertifiedFetchBodyPersistenceDropGuard {
    fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for CertifiedFetchBodyPersistenceDropGuard {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
struct GuardedCertifiedFetchBodyPersistenceCompletion {
    completion: Option<CertifiedFetchBodyPersistenceCompletion>,
    drop_guard: CertifiedFetchBodyPersistenceDropGuard,
}
struct RecoveredDecisionFetchBodyCompletionDropGuardV1 {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl RecoveredDecisionFetchBodyCompletionDropGuardV1 {
    fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for RecoveredDecisionFetchBodyCompletionDropGuardV1 {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
struct GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1 {
    completion: Option<RecoveredDecisionFetchBodyPersistenceCompletionV1>,
    drop_guard: RecoveredDecisionFetchBodyCompletionDropGuardV1,
}
impl GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1 {
    fn new(
        completion: RecoveredDecisionFetchBodyPersistenceCompletionV1,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            completion: Some(completion),
            drop_guard: RecoveredDecisionFetchBodyCompletionDropGuardV1::new(output_guard),
        }
    }
    fn completion(&self) -> &RecoveredDecisionFetchBodyPersistenceCompletionV1 {
        self.completion
            .as_ref()
            .expect("armed recovered Decision Fetch completion retains its payload")
    }
    fn acknowledge_after_publication(mut self) {
        let _completion = self
            .completion
            .take()
            .expect("settled recovered Decision Fetch consumes its completion once");
        self.drop_guard.disarm();
    }
}
impl GuardedCertifiedFetchBodyPersistenceCompletion {
    fn new(
        completion: CertifiedFetchBodyPersistenceCompletion,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            completion: Some(completion),
            drop_guard: CertifiedFetchBodyPersistenceDropGuard::new(output_guard),
        }
    }
    fn completion(&self) -> &CertifiedFetchBodyPersistenceCompletion {
        self.completion
            .as_ref()
            .expect("armed certified-Fetch completion retains its payload")
    }
    fn into_completion(mut self) -> CertifiedFetchBodyPersistenceCompletion {
        let completion = self
            .completion
            .take()
            .expect("prepared WorkAck consumes the guarded completion once");
        self.drop_guard.disarm();
        completion
    }
}
struct RecoveredDecisionApplyCompletionDropGuardV1 {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl RecoveredDecisionApplyCompletionDropGuardV1 {
    fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for RecoveredDecisionApplyCompletionDropGuardV1 {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
struct GuardedRecoveredDecisionApplyWorkerResultV1 {
    result: Option<RecoveredDecisionApplyWorkerResultV1>,
    drop_guard: RecoveredDecisionApplyCompletionDropGuardV1,
}
struct RecoveredLifecycleSignCompletionDropGuardV1 {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl RecoveredLifecycleSignCompletionDropGuardV1 {
    fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for RecoveredLifecycleSignCompletionDropGuardV1 {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
struct GuardedRecoveredLifecycleSignWorkerResultV1 {
    result: RecoveredLifecycleSignWorkerResultV1,
    drop_guard: RecoveredLifecycleSignCompletionDropGuardV1,
}
impl GuardedRecoveredLifecycleSignWorkerResultV1 {
    fn new(
        result: RecoveredLifecycleSignWorkerResultV1,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            result,
            drop_guard: RecoveredLifecycleSignCompletionDropGuardV1::new(output_guard),
        }
    }
    const fn result(&self) -> &RecoveredLifecycleSignWorkerResultV1 {
        &self.result
    }
    fn acknowledge_after_publication(mut self) {
        self.drop_guard.disarm();
    }
}
impl GuardedRecoveredDecisionApplyWorkerResultV1 {
    fn new(
        result: RecoveredDecisionApplyWorkerResultV1,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            result: Some(result),
            drop_guard: RecoveredDecisionApplyCompletionDropGuardV1::new(output_guard),
        }
    }
    fn result(&self) -> &RecoveredDecisionApplyWorkerResultV1 {
        self.result
            .as_ref()
            .expect("armed recovered Decision Apply completion retains its result")
    }
    fn into_result(mut self) -> RecoveredDecisionApplyWorkerResultV1 {
        let result = self
            .result
            .take()
            .expect("settled recovered Decision Apply consumes its result once");
        self.drop_guard.disarm();
        result
    }
    fn into_retry_parts(
        self,
    ) -> (
        RecoveredDecisionApplyWorkerResultV1,
        RecoveredDecisionApplyCompletionDropGuardV1,
    ) {
        let Self { result, drop_guard } = self;
        (
            result.expect("armed recovered Decision Apply completion retains its result"),
            drop_guard,
        )
    }
    fn from_retry_parts(
        result: RecoveredDecisionApplyWorkerResultV1,
        drop_guard: RecoveredDecisionApplyCompletionDropGuardV1,
    ) -> Self {
        Self {
            result: Some(result),
            drop_guard,
        }
    }
}
/// Move-only recovered-Apply acknowledgement consumed after durable settlement;
/// Drop closes output without releasing its queue index.
#[must_use = "recovered Decision Apply work remains indexed until owner settlement"]
struct RecoveredDecisionApplyWorkAckV1 {
    queue: Arc<V2IoCommandQueue>,
    output_guard: Arc<ConsensusOutputGuard>,
    key: RecoveredDecisionApplyDispatchKeyV1,
    armed: bool,
}
impl RecoveredDecisionApplyWorkAckV1 {
    fn acknowledge(mut self) {
        self.queue.acknowledge_recovered_decision_apply(self.key);
        self.queue
            .admission
            .acknowledge_recovered_decision_apply_completion(self.key);
        self.armed = false;
    }
    fn acknowledge_retry_publication(mut self) {
        self.armed = false;
    }
}
impl Drop for RecoveredDecisionApplyWorkAckV1 {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
/// Guarded worker result which can be consumed only after lifecycle settlement.
#[must_use = "the recovered Decision Apply result still requires owner settlement"]
pub(in crate::sumeragi) struct PreparedRecoveredDecisionApplyCompletionV1 {
    guarded: Box<GuardedRecoveredDecisionApplyWorkerResultV1>,
    work_ack: RecoveredDecisionApplyWorkAckV1,
}
/// Guarded recovered-Sign completion with only a fixed adapter-private preview;
/// abandonment closes output while its command owner remains recoverable.
#[must_use = "recovered Sign completion must enter restart-closed owner settlement"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct PreparedRecoveredLifecycleSignCompletionV1 {
    guarded: Box<GuardedRecoveredLifecycleSignWorkerResultV1>,
    queue: Arc<V2IoCommandQueue>,
}
/// Guarded durable recovered-Fetch body parked for restart-closed Store settlement.
#[must_use = "recovered Decision Fetch persistence remains guarded and indexed"]
pub(in crate::sumeragi) struct PreparedRecoveredDecisionFetchBodyCompletionV1 {
    guarded: Box<GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1>,
    queue: Arc<V2IoCommandQueue>,
}
/// Guarded lifecycle Serve completion retained through LedgerV1 and reply delivery.
#[must_use = "Certified-Serve completion must be settled and acknowledged"]
pub(in crate::sumeragi) struct PreparedLifecycleCertifiedServeCompletionV1 {
    guarded: Box<GuardedLifecycleCertifiedServeWorkerResultV1>,
    queue: Arc<V2IoCommandQueue>,
}
impl PreparedLifecycleCertifiedServeCompletionV1 {
    fn new(
        guarded: Box<GuardedLifecycleCertifiedServeWorkerResultV1>,
        queue: Arc<V2IoCommandQueue>,
        ownership_position: usize,
    ) -> Option<Self> {
        let result = guarded.result();
        queue
            .transfer_lifecycle_certified_serve_completion(
                result.lifecycle_ordinal(),
                result.request_hash(),
                ownership_position,
            )
            .then_some(Self { guarded, queue })
    }

    /// Publish the LedgerV1 terminal, deliver the response, and retire its owner.
    pub(in crate::sumeragi) fn settle_deliver_and_acknowledge(
        mut self,
        owner: &mut crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1,
        services: &ProductionV2Services,
    ) -> Result<(), String> {
        {
            let result = self
                .guarded
                .result
                .as_mut()
                .expect("armed lifecycle Certified-Serve completion retains its result");
            let body_readback = result.body_readback.take().ok_or_else(|| {
                "lifecycle Certified-Serve completion lost its body readback".to_owned()
            })?;
            let authority = result.task.authority.take().ok_or_else(|| {
                "lifecycle Certified-Serve completion lost its terminal authority".to_owned()
            })?;
            match authority {
                LifecycleCertifiedServeTaskAuthorityV1::Claimed(lease) => owner
                    .settle_certified_serve_worker_completed(
                        lease,
                        &result.task.authenticated,
                        body_readback,
                        &result.response,
                    )
                    .map_err(|_| {
                        "lifecycle Certified-Serve terminal settlement failed".to_owned()
                    })?,
                LifecycleCertifiedServeTaskAuthorityV1::TerminalReplay(authorization) => owner
                    .verify_certified_serve_terminal_replay(
                        authorization,
                        &result.task.authenticated,
                        body_readback,
                        &result.response,
                    )
                    .map_err(|_| {
                        "lifecycle Certified-Serve terminal replay verification failed".to_owned()
                    })?,
            }
        }
        let result = self.guarded.result();
        services.post_to_peer_on_reply_routes(
            result.task.recipient.clone(),
            result.task.reply_routes.clone(),
            result.task.ingress_ownership.clone(),
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::CertifiedBodyResponse(
                result.response.clone(),
            )),
        )?;
        self.queue.acknowledge_lifecycle_certified_serve(
            result.lifecycle_ordinal(),
            result.request_hash(),
        );
        let _ = (*self.guarded).into_result();
        Ok(())
    }
}
impl PreparedRecoveredDecisionFetchBodyCompletionV1 {
    fn new(
        guarded: Box<GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1>,
        queue: Arc<V2IoCommandQueue>,
        ownership_position: usize,
    ) -> Option<Self> {
        let key = guarded.completion().dispatch_key();
        let state_is_exact = {
            let state = queue.lock();
            state
                .recovered_decision_fetch_bodies
                .get(&key)
                .is_some_and(|tracked| {
                    tracked.state == V2IoWorkState::CompletionPending
                        && tracked.id == guarded.completion().id()
                        && tracked.response_hash == guarded.completion().response_hash()
                })
        };
        (state_is_exact
            && queue
                .admission
                .transfer_recovered_decision_fetch_completion_at(key, ownership_position))
        .then_some(Self { guarded, queue })
    }
    /// Borrow the opaque durable completion for fixed settlement projections.
    pub(in crate::sumeragi) fn completion(
        &self,
    ) -> &RecoveredDecisionFetchBodyPersistenceCompletionV1 {
        self.guarded.completion()
    }
    /// Retire the exact command index and disarm restart closure after LedgerV1 publication.
    pub(in crate::sumeragi) fn acknowledge_after_publication(self) {
        let key = self.guarded.completion().dispatch_key();
        let id = self.guarded.completion().id();
        let response_hash = self.guarded.completion().response_hash();
        self.queue
            .acknowledge_recovered_decision_fetch_body(key, id, response_hash);
        self.guarded.acknowledge_after_publication();
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedRecoveredLifecycleSignCompletionV1 {
    fn new(
        guarded: Box<GuardedRecoveredLifecycleSignWorkerResultV1>,
        queue: Arc<V2IoCommandQueue>,
        ownership_position: usize,
    ) -> Option<Self> {
        queue
            .transfer_recovered_lifecycle_sign_completion(
                guarded.result().dispatch_key(),
                ownership_position,
            )
            .then_some(Self { guarded, queue })
    }
    /// Clone a revalidated opaque result for private adapter preview while the
    /// original remains guarded until LedgerV1 publication.
    pub(in crate::sumeragi) fn project_adapter_completion_authority(
        &self,
    ) -> Option<RecoveredLifecycleSignAdapterCompletionAuthorityV1> {
        let result = self.guarded.result();
        if !result.is_exact() {
            return None;
        }
        Some(RecoveredLifecycleSignAdapterCompletionAuthorityV1 {
            key: result.dispatch_key(),
            tag: result.task.tag,
            request: result.task.request.clone(),
            signature: result.signature.clone(),
            outbound_payload: result.outbound_payload.clone(),
        })
    }
    /// Retire the command owner after durable Broadcast publication and all
    /// volatile assertion-only tails, then disarm restart closure.
    pub(in crate::sumeragi) fn acknowledge_after_publication(self) {
        let key = self.guarded.result().dispatch_key();
        self.queue.acknowledge_recovered_lifecycle_sign(key);
        self.guarded.acknowledge_after_publication();
    }
}
/// Result of atomically returning one guarded missing-sidecar Apply to the worker FIFO.
#[must_use = "an unavailable recovered Apply retry still owns its guarded completion"]
pub(in crate::sumeragi) enum RecoveredDecisionApplyDeferredRetryV1 {
    /// The same dispatch key and task were republished to the dedicated worker queue.
    Requeued,
    /// Consensus queue capacity is unavailable; the complete guarded result remains owned.
    Unavailable(PreparedRecoveredDecisionApplyCompletionV1),
    /// The dedicated queue index no longer matched the retained completion.
    RestartRequired,
}
impl PreparedRecoveredDecisionApplyCompletionV1 {
    /// Compare service queue, output guard, and recovery owner without releasing
    /// guarded completion or process-local dependencies.
    pub(in crate::sumeragi) fn authorizes_sidecar_owner(
        &self,
        services: &ProductionV2Services,
        lane_work: &V2LaneWorkAdapter,
    ) -> bool {
        services.owns_recovered_decision_apply_queue(&self.work_ack.queue)
            && Arc::ptr_eq(&services.output_guard, &self.work_ack.output_guard)
            && services.matches_lifecycle_lane_work(lane_work)
    }
    /// Borrow the exact Applied/Deferred result while the command remains indexed.
    pub(in crate::sumeragi) fn result(&self) -> &RecoveredDecisionApplyWorkerResultV1 {
        self.guarded.result()
    }
    /// Release the dedicated queue index after the owner durably settled this result.
    ///
    /// This is intentionally not a generic worker acknowledgement: its only
    /// caller is the recovered Decision Apply owner settlement transaction.
    pub(in crate::sumeragi) fn acknowledge_after_owner_settlement(
        self,
    ) -> RecoveredDecisionApplyWorkerResultV1 {
        let Self { guarded, work_ack } = self;
        work_ack.acknowledge();
        (*guarded).into_result()
    }
    /// Republish a `CompletionPending` sidecar task under its existing owner,
    /// reserving/enqueueing before disarming guards; mismatch requires restart.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn retry_deferred(self) -> RecoveredDecisionApplyDeferredRetryV1 {
        let Self { guarded, work_ack } = self;
        let (result, mut completion_guard) = (*guarded).into_retry_parts();
        let RecoveredDecisionApplyWorkerResultV1::Deferred { task, reference } = result else {
            drop(work_ack);
            drop(completion_guard);
            return RecoveredDecisionApplyDeferredRetryV1::RestartRequired;
        };
        match work_ack.queue.retry_recovered_decision_apply(task) {
            Ok(()) => {
                work_ack.acknowledge_retry_publication();
                completion_guard.disarm();
                RecoveredDecisionApplyDeferredRetryV1::Requeued
            }
            Err(RecoveredDecisionApplyRetryQueueErrorV1::Unavailable(task)) => {
                RecoveredDecisionApplyDeferredRetryV1::Unavailable(Self {
                    guarded: Box::new(
                        GuardedRecoveredDecisionApplyWorkerResultV1::from_retry_parts(
                            RecoveredDecisionApplyWorkerResultV1::Deferred { task, reference },
                            completion_guard,
                        ),
                    ),
                    work_ack,
                })
            }
            Err(RecoveredDecisionApplyRetryQueueErrorV1::InvalidOwner(_task)) => {
                drop(work_ack);
                drop(completion_guard);
                RecoveredDecisionApplyDeferredRetryV1::RestartRequired
            }
        }
    }
}
struct LifecycleCertifiedServeCompletionDropGuardV1 {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl LifecycleCertifiedServeCompletionDropGuardV1 {
    fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for LifecycleCertifiedServeCompletionDropGuardV1 {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
struct GuardedLifecycleCertifiedServeWorkerResultV1 {
    result: Option<LifecycleCertifiedServeWorkerResultV1>,
    drop_guard: LifecycleCertifiedServeCompletionDropGuardV1,
}
impl GuardedLifecycleCertifiedServeWorkerResultV1 {
    fn new(
        result: LifecycleCertifiedServeWorkerResultV1,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            result: Some(result),
            drop_guard: LifecycleCertifiedServeCompletionDropGuardV1::new(output_guard),
        }
    }
    fn result(&self) -> &LifecycleCertifiedServeWorkerResultV1 {
        self.result
            .as_ref()
            .expect("armed lifecycle Certified-Serve completion retains its result")
    }
    fn into_result(mut self) -> LifecycleCertifiedServeWorkerResultV1 {
        let result = self
            .result
            .take()
            .expect("settled lifecycle Certified-Serve consumes its result once");
        self.drop_guard.disarm();
        result
    }
}
enum V2IoCompletion {
    Signature {
        work_id: EffectWorkId,
        signature: Vec<u8>,
        outbound_payload: Option<EncodedV2Payload>,
    },
    Stored(BodyStoreCompletion),
    CertifiedFetchBodyPersisted(GuardedCertifiedFetchBodyPersistenceCompletion),
    RecoveredDecisionFetchBodyPersisted(
        Box<GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1>,
    ),
    Validated(BodyValidationCompletion),
    Applied(Box<DurableApplyCompletion>),
    RecoveredDecisionApply(Box<GuardedRecoveredDecisionApplyWorkerResultV1>),
    RecoveredLifecycleSign(Box<GuardedRecoveredLifecycleSignWorkerResultV1>),
    LifecycleCertifiedServe(Box<GuardedLifecycleCertifiedServeWorkerResultV1>),
    ApplyDeferred {
        work_id: EffectWorkId,
        reference: CertifiedMergeLedgerReference,
    },
    #[cfg(test)]
    AuxiliaryNoop,
    CandidateLoaded(LockedCandidateLoad),
    CandidateLoadUnavailable {
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
    },
    CandidateLoadFailed {
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
        reason: String,
    },
    Retired,
    RetirementFailed(String),
    RecoveryRequired(String),
    Failed(String),
}
impl V2IoCompletion {
    fn recovered_decision_apply_key(&self) -> Option<RecoveredDecisionApplyDispatchKeyV1> {
        match self {
            Self::RecoveredDecisionApply(guarded) => Some(guarded.result().dispatch_key()),
            _ => None,
        }
    }
    fn recovered_lifecycle_sign_key(&self) -> Option<RecoveredLifecycleSignDispatchKeyV1> {
        match self {
            Self::RecoveredLifecycleSign(guarded) => Some(guarded.result().dispatch_key()),
            _ => None,
        }
    }
    fn recovered_decision_fetch_key(&self) -> Option<RecoveredDecisionFetchDispatchKeyV1> {
        match self {
            Self::RecoveredDecisionFetchBodyPersisted(guarded) => {
                Some(guarded.completion().dispatch_key())
            }
            _ => None,
        }
    }
    fn lifecycle_certified_serve_ordinal(&self) -> Option<u128> {
        match self {
            Self::LifecycleCertifiedServe(guarded) => Some(guarded.result().lifecycle_ordinal()),
            _ => None,
        }
    }
    // `false` variants never enqueue a reducer completion. They operate only
    // on non-reducer effect, network, or service state (or report a terminal
    // failure), so they may be serviced behind one retained runtime result
    // without reordering any reducer-visible completion.
    const fn requires_runtime_capacity(&self) -> bool {
        matches!(
            self,
            Self::Signature { .. }
                | Self::Stored(_)
                | Self::Validated(_)
                | Self::Applied(_)
                | Self::RecoveredDecisionApply(_)
                | Self::RecoveredLifecycleSign(_)
        )
    }
    fn acknowledgement(&self) -> V2IoCompletionAcknowledgement {
        match self {
            Self::Signature { work_id, .. } | Self::ApplyDeferred { work_id, .. } => {
                V2IoCompletionAcknowledgement::Work(*work_id)
            }
            Self::Stored(completion) => V2IoCompletionAcknowledgement::Work(completion.work_id()),
            Self::CertifiedFetchBodyPersisted(_) => {
                V2IoCompletionAcknowledgement::LifecycleWorkRetained
            }
            Self::RecoveredDecisionFetchBodyPersisted(_) => {
                V2IoCompletionAcknowledgement::RecoveredDecisionFetchRetained
            }
            Self::Validated(completion) => {
                V2IoCompletionAcknowledgement::Work(completion.work_id())
            }
            Self::Applied(completion) => V2IoCompletionAcknowledgement::Work(completion.work_id()),
            Self::RecoveredDecisionApply(_) => {
                V2IoCompletionAcknowledgement::RecoveredDecisionApplyRetained
            }
            Self::RecoveredLifecycleSign(_) => {
                V2IoCompletionAcknowledgement::RecoveredLifecycleSignRetained
            }
            Self::LifecycleCertifiedServe(_) => {
                V2IoCompletionAcknowledgement::LifecycleServeRetained
            }
            Self::CandidateLoaded(_)
            | Self::CandidateLoadUnavailable { .. }
            | Self::CandidateLoadFailed { .. }
            | Self::Retired
            | Self::RetirementFailed(_)
            | Self::RecoveryRequired(_)
            | Self::Failed(_) => V2IoCompletionAcknowledgement::Untracked,
            #[cfg(test)]
            Self::AuxiliaryNoop => V2IoCompletionAcknowledgement::Untracked,
        }
    }
}
enum V2IoCompletionAcknowledgement {
    Work(EffectWorkId),
    LifecycleWorkRetained,
    RecoveredDecisionApplyRetained,
    RecoveredLifecycleSignRetained,
    RecoveredDecisionFetchRetained,
    LifecycleServeRetained,
    Untracked,
}
/// Move-only persistence acknowledgement retaining `CompletionPending` work so
/// repeated selector probes coalesce until Phase B consumes ingress.
#[must_use = "the exact command index must remain occupied until Phase B commits"]
pub(in crate::sumeragi) struct CertifiedFetchBodyPersistenceWorkAck {
    queue: Arc<V2IoCommandQueue>,
    output_guard: Arc<ConsensusOutputGuard>,
    work_id: EffectWorkId,
    descriptor: V2IoWorkDescriptor,
    armed: bool,
}
impl CertifiedFetchBodyPersistenceWorkAck {
    /// Release the exact command index only in the post-dequeue infallible tail.
    pub(in crate::sumeragi) fn commit(mut self) {
        self.queue
            .acknowledge_exact_lifecycle_completion(self.work_id, &self.descriptor);
        self.armed = false;
    }
}
impl Drop for CertifiedFetchBodyPersistenceWorkAck {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
/// Persisted body plus its still-indexed exact command owner.
#[must_use = "the persisted response and duplicate fence require Phase-B consumption"]
pub(crate) struct PreparedCertifiedFetchBodyPersistenceCompletion {
    completion: CertifiedFetchBodyPersistenceCompletion,
    work_ack: CertifiedFetchBodyPersistenceWorkAck,
}
impl PreparedCertifiedFetchBodyPersistenceCompletion {
    /// Return the still-indexed existing executor work identity for diagnostics.
    pub(in crate::sumeragi) const fn work_id(&self) -> EffectWorkId {
        self.completion.work_id()
    }
    /// Split two opaque move-only authorities for the sealed composite transaction.
    pub(in crate::sumeragi) fn into_parts(
        self,
    ) -> (
        CertifiedFetchBodyPersistenceCompletion,
        CertifiedFetchBodyPersistenceWorkAck,
    ) {
        (self.completion, self.work_ack)
    }
    /// Rejoin an unchanged pre-dequeue completion after a retryable failure.
    pub(in crate::sumeragi) fn from_parts(
        completion: CertifiedFetchBodyPersistenceCompletion,
        work_ack: CertifiedFetchBodyPersistenceWorkAck,
    ) -> Self {
        Self {
            completion,
            work_ack,
        }
    }
}
/// Typed outcome of the ordinary bounded completion drain.
///
/// A persisted certified-Fetch body is returned directly to the serialized
/// caller; it is never parked in a service-side flag, latch, or second queue.
#[must_use = "a persisted certified-Fetch body must be consumed by its coordinator owner"]
pub(crate) struct V2CompletionDrainOutcome {
    serviced: usize,
    certified_fetch_body: Option<PreparedCertifiedFetchBodyPersistenceCompletion>,
}
impl V2CompletionDrainOutcome {
    /// Split the count from the move-only lifecycle completion.
    pub(crate) fn into_parts(
        self,
    ) -> (
        usize,
        Option<PreparedCertifiedFetchBodyPersistenceCompletion>,
    ) {
        (self.serviced, self.certified_fetch_body)
    }
}
/// Owner-only drain of at most one guarded recovered Sign completion.
#[must_use = "a recovered Sign drain remains parked under its lifecycle owner"]
pub(in crate::sumeragi) struct RecoveredLifecycleSignCompletionDrainV1 {
    completion: Option<PreparedRecoveredLifecycleSignCompletionV1>,
}
/// Owner-only drain of at most one lifecycle-owned Certified-Serve completion.
#[must_use = "a Certified-Serve completion must remain parked under its lifecycle owner"]
pub(in crate::sumeragi) struct LifecycleCertifiedServeCompletionDrainV1 {
    completion: Option<PreparedLifecycleCertifiedServeCompletionV1>,
}

/// Opaque result of taking the physical completion head exactly once.
///
/// Ordinary I/O and local reconstruction work is never exposed. An ordinary
/// I/O head is restored into the service's sole held slot before
/// `PassThrough` returns, so the ordinary drain observes the same FIFO item.
/// Recovered variants transfer only their guarded, class-specific owner.
#[allow(variant_size_differences)]
#[must_use = "a selected recovered completion must remain lifecycle-owned"]
pub(in crate::sumeragi) enum RecoveredLifecycleCompletionTakeV1 {
    /// No physical I/O completion is currently available.
    None,
    /// The ordinary completion owner must service the current turn.
    PassThrough,
    /// The exact recovered Decision Apply completion left the FIFO owner.
    Apply(PreparedRecoveredDecisionApplyCompletionV1),
    /// The exact recovered Sign completion left the FIFO owner.
    Sign(PreparedRecoveredLifecycleSignCompletionV1),
    /// The exact persisted recovered Decision Fetch body left the FIFO owner.
    DecisionFetch(PreparedRecoveredDecisionFetchBodyCompletionV1),
    /// The exact lifecycle-owned Serve result left the FIFO owner.
    CertifiedServe(PreparedLifecycleCertifiedServeCompletionV1),
}

impl LifecycleCertifiedServeCompletionDrainV1 {
    /// Consume the drain result into its optional guarded Serve completion.
    pub(in crate::sumeragi) fn into_completion(
        self,
    ) -> Option<PreparedLifecycleCertifiedServeCompletionV1> {
        self.completion
    }
}
impl RecoveredLifecycleSignCompletionDrainV1 {
    /// Consume the drain into its optional opaque guarded completion.
    pub(in crate::sumeragi) fn into_completion(
        self,
    ) -> Option<PreparedRecoveredLifecycleSignCompletionV1> {
        self.completion
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
struct PostFinalityCleanupJob {
    identity: CleanupWorkerIdentity,
    bodies: V2BodyRetirementJob,
    chunk_root: PathBuf,
}
const POST_FINALITY_CLEANUP_QUEUE_CAPACITY: usize = 4;
#[derive(Clone)]
struct V2CleanupSubmission {
    sender: mpsc::SyncSender<PostFinalityCleanupJob>,
}
impl V2CleanupSubmission {
    fn try_submit(&self, job: PostFinalityCleanupJob) -> Result<(), String> {
        let identity = job.identity;
        match self.sender.try_send(job) {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Full(_)) => {
                let reason =
                    "bounded Sumeragi v2 cleanup queue is full; retaining finalized local files";
                report_post_finality_cleanup_warning(
                    identity,
                    PostFinalityCleanupTarget::CleanupWorker,
                    reason,
                );
                Err(reason.to_owned())
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                let reason =
                    "Sumeragi v2 cleanup worker is unavailable; retaining finalized local files";
                report_post_finality_cleanup_warning(
                    identity,
                    PostFinalityCleanupTarget::CleanupWorker,
                    reason,
                );
                Err(reason.to_owned())
            }
        }
    }
}
/// Runner-owned cleanup janitor: consensus only uses bounded non-blocking
/// enqueue, and stalled work remains for startup reconciliation.
pub(crate) struct V2CleanupSupervisor {
    submission: Option<V2CleanupSubmission>,
    join: Option<thread::JoinHandle<()>>,
}
impl Default for V2CleanupSupervisor {
    fn default() -> Self {
        Self::with_capacity(
            NonZeroUsize::new(POST_FINALITY_CLEANUP_QUEUE_CAPACITY)
                .expect("cleanup queue capacity is non-zero"),
        )
    }
}
impl V2CleanupSupervisor {
    fn with_capacity(capacity: NonZeroUsize) -> Self {
        let (sender, receiver) = mpsc::sync_channel(capacity.get());
        let submission = V2CleanupSubmission { sender };
        let join = match super::sumeragi_thread_builder("sumeragi-v2-cleanup").spawn(move || {
            while let Ok(job) = receiver.recv() {
                execute_post_finality_cleanup(job);
            }
        }) {
            Ok(join) => Some(join),
            Err(error) => {
                iroha_logger::warn!(
                    cleanup_target = PostFinalityCleanupTarget::CleanupWorker.as_str(),
                    reason = %error,
                    "failed to start the bounded Sumeragi v2 cleanup worker"
                );
                None
            }
        };
        Self {
            submission: Some(submission),
            join,
        }
    }
    fn submission(&self) -> V2CleanupSubmission {
        self.submission
            .as_ref()
            .expect("cleanup submission exists until supervisor drop")
            .clone()
    }
    /// Reap a terminated janitor without ever joining a running thread.
    pub(crate) fn reap_finished(&mut self) {
        if self
            .join
            .as_ref()
            .is_some_and(thread::JoinHandle::is_finished)
        {
            let join = self.join.take().expect("finished cleanup worker exists");
            if join.join().is_err() {
                iroha_logger::warn!(
                    cleanup_target = PostFinalityCleanupTarget::CleanupWorker.as_str(),
                    reason = "bounded Sumeragi v2 cleanup worker panicked",
                    "Sumeragi v2 finalized with retained local cleanup state"
                );
            }
        }
    }
}
impl Drop for V2CleanupSupervisor {
    fn drop(&mut self) {
        self.submission.take();
        if self
            .join
            .as_ref()
            .is_some_and(thread::JoinHandle::is_finished)
        {
            let join = self.join.take().expect("finished cleanup worker exists");
            let _ = join.join();
        }
    }
}
fn execute_post_finality_cleanup(job: PostFinalityCleanupJob) {
    if let Err(error) = job.bodies.execute() {
        report_post_finality_cleanup_warning(
            job.identity,
            PostFinalityCleanupTarget::DurableBodies,
            &error.to_string(),
        );
    }
    match std::fs::remove_dir_all(&job.chunk_root) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => report_post_finality_cleanup_warning(
            job.identity,
            PostFinalityCleanupTarget::PayloadChunks,
            &format!(
                "failed to remove Sumeragi v2 chunk root {}: {error}",
                job.chunk_root.display()
            ),
        ),
    }
}
fn report_post_finality_cleanup_warning(
    identity: CleanupWorkerIdentity,
    target: PostFinalityCleanupTarget,
    reason: &str,
) {
    iroha_logger::warn!(
        height = identity.height,
        context_id = ?identity.context_id,
        block_hash = %identity.block_hash,
        cleanup_target = target.as_str(),
        reason,
        "Sumeragi v2 finalized with retained local cleanup state"
    );
}
impl V2IoHandle {
    fn spawn(
        body_store: V2BodyStore,
        apply_service: V2ApplyService,
        context: wire::HeightContext,
        key_pair: KeyPair,
        local_validator: Option<wire::ValidatorIndex>,
        auxiliary_queue_capacity: usize,
        consensus_queue_capacity: usize,
        observer_serve_capacity: usize,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<Self, String> {
        let admission = Arc::new(V2IoAdmission::new(
            auxiliary_queue_capacity,
            consensus_queue_capacity,
        )?);
        let capacity = admission.capacity();
        if observer_serve_capacity == 0 {
            return Err("Sumeragi v2 observer Serve capacity must be non-zero".to_owned());
        }
        let (command_tx, command_rx) =
            build_v2_io_command_channel(capacity, Arc::clone(&admission));
        let (completion_tx, completion_rx) = mpsc::sync_channel(capacity);
        let allow_finalized_disconnect = Arc::new(AtomicBool::new(false));
        let worker_allow_finalized_disconnect = Arc::clone(&allow_finalized_disconnect);
        let worker_admission = Arc::clone(&admission);
        let join = super::sumeragi_thread_builder("sumeragi-v2-io")
            .spawn(move || {
                // A local guard drops before the closure environment releases
                // command/completion channels, closing output first on panic
                // or an implicit producer disconnect.
                let mut worker_failure_guard = V2IoWorkerFailureGuard::new(
                    Arc::clone(&output_guard),
                    worker_allow_finalized_disconnect,
                );
                let mut body_store = Some(body_store);
                while let Ok(command) = command_rx.recv() {
                    let work_id = command.work_id();
                    let recovered_decision_apply_key = command.recovered_decision_apply_key();
                    let recovered_lifecycle_sign_key = command.recovered_lifecycle_sign_key();
                    let recovered_decision_fetch_key = command.recovered_decision_fetch_key();
                    let runtime_lifecycle_ordinal = command.runtime_lifecycle_ordinal();
                    match command {
                        V2IoCommand::Retire(retire) => {
                            let Some(completion) = execute_retire_io_command(&output_guard, || {
                                let bodies = body_store
                                    .take()
                                    .expect("Retire consumes the live height-local body store")
                                    .into_retirement_job(&retire.receipt)
                                    .map_err(|error| error.to_string())?;
                                retire.cleanup.try_submit(PostFinalityCleanupJob {
                                    identity: CleanupWorkerIdentity::from_receipt(&retire.receipt),
                                    bodies,
                                    chunk_root: retire.chunk_root,
                                })
                            }) else {
                                break;
                            };
                            let _ = send_tracked_completion(
                                &completion_tx,
                                &worker_admission,
                                completion,
                            );
                            worker_failure_guard.disarm();
                            break;
                        }
                        V2IoCommand::Shutdown => {
                            worker_failure_guard.disarm();
                            break;
                        }
                        V2IoCommand::LoadCandidate {
                            acquisition_id,
                            subject,
                        } => {
                            let completion = match load_candidate_body(
                                body_store
                                    .as_ref()
                                    .expect("body store remains live before Retire"),
                                acquisition_id,
                                subject,
                            ) {
                                Ok(Some(loaded)) => V2IoCompletion::CandidateLoaded(loaded),
                                Ok(None) => V2IoCompletion::CandidateLoadUnavailable {
                                    acquisition_id,
                                    subject,
                                },
                                Err(reason) => V2IoCompletion::CandidateLoadFailed {
                                    acquisition_id,
                                    subject,
                                    reason,
                                },
                            };
                            send_completion(&completion_tx, &worker_admission, Ok(completion));
                        }
                        command => {
                            let completion = execute_fail_stop_io_command(&output_guard, || {
                                match command {
                                    V2IoCommand::Sign {
                                        task,
                                        restore_outbound_payload,
                                    } => sign_consensus_task(
                                        body_store
                                            .as_ref()
                                            .expect("body store remains live before Retire"),
                                        &context,
                                        &key_pair,
                                        task,
                                        restore_outbound_payload,
                                    ),
                                    V2IoCommand::Store(task) => body_store
                                        .as_mut()
                                        .expect("body store remains live before Retire")
                                        .execute_store_task(&task)
                                        .map(V2IoCompletion::Stored)
                                        .map_err(|error| error.to_string()),
                                    V2IoCommand::PersistCertifiedFetchBody(task) => task
                                        .persist(
                                            body_store
                                                .as_mut()
                                                .expect("body store remains live before Retire"),
                                        )
                                        .map(|completion| {
                                            V2IoCompletion::CertifiedFetchBodyPersisted(
                                                GuardedCertifiedFetchBodyPersistenceCompletion::new(
                                                    completion,
                                                    Arc::clone(&output_guard),
                                                ),
                                            )
                                        })
                                        .map_err(|(error, _task)| error.to_string()),
                                    V2IoCommand::PersistRecoveredDecisionFetchBody(task) => task
                                        .persist(
                                            body_store
                                                .as_mut()
                                                .expect("body store remains live before Retire"),
                                        )
                                        .map(|completion| {
                                            V2IoCompletion::RecoveredDecisionFetchBodyPersisted(
                                                Box::new(
                                                    GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1::new(
                                                        completion,
                                                        Arc::clone(&output_guard),
                                                    ),
                                                ),
                                            )
                                        })
                                        .map_err(|(error, _task)| error.to_string()),
                                    V2IoCommand::Validate(task) => body_store
                                        .as_mut()
                                        .expect("body store remains live before Retire")
                                        .execute_validation_task(&task, |body| {
                                            apply_service.validate_candidate(&context, body)
                                        })
                                        .map(V2IoCompletion::Validated)
                                        .map_err(|error| error.to_string()),
                                    V2IoCommand::Apply(task) => match apply_service.execute(
                                        &context,
                                        body_store
                                            .as_mut()
                                            .expect("body store remains live before Retire"),
                                        &task,
                                    ) {
                                        Ok(completion) => {
                                            Ok(V2IoCompletion::Applied(Box::new(completion)))
                                        }
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
                                    V2IoCommand::RecoveredDecisionApply(task) => apply_service
                                        .execute_recovered_decision_apply(
                                            &context,
                                            body_store
                                                .as_mut()
                                                .expect("body store remains live before Retire"),
                                            task,
                                        )
                                        .map(|result| {
                                            V2IoCompletion::RecoveredDecisionApply(
                                                Box::new(GuardedRecoveredDecisionApplyWorkerResultV1::new(
                                                    result,
                                                    Arc::clone(&output_guard),
                                                )),
                                            )
                                        })
                                        .or_else(|error| {
                                            if error.requires_restart_recovery() {
                                                Ok(V2IoCompletion::RecoveryRequired(
                                                    error.to_string(),
                                                ))
                                            } else {
                                                Err(error.to_string())
                                            }
                                        }),
                                    V2IoCommand::RecoveredLifecycleSign(task) => {
                                        sign_recovered_lifecycle_task(
                                            body_store
                                                .as_ref()
                                                .expect("body store remains live before Retire"),
                                            &context,
                                            &key_pair,
                                            task,
                                        )
                                        .map(|result| {
                                            V2IoCompletion::RecoveredLifecycleSign(Box::new(
                                                GuardedRecoveredLifecycleSignWorkerResultV1::new(
                                                    result,
                                                    Arc::clone(&output_guard),
                                                ),
                                            ))
                                        })
                                    }
                                    V2IoCommand::LifecycleCertifiedServe(task) => {
                                        serve_lifecycle_certified_body(
                                            body_store
                                                .as_ref()
                                                .expect("body store remains live before Retire"),
                                            &key_pair,
                                            local_validator,
                                            task,
                                        )
                                        .map(|result| {
                                            V2IoCompletion::LifecycleCertifiedServe(Box::new(
                                                GuardedLifecycleCertifiedServeWorkerResultV1::new(
                                                    result,
                                                    Arc::clone(&output_guard),
                                                ),
                                            ))
                                        })
                                    }
                                    V2IoCommand::LoadCandidate { .. }
                                    | V2IoCommand::Retire(_)
                                    | V2IoCommand::Shutdown => {
                                        unreachable!(
                                            "cleanup commands handled before fail-stop I/O"
                                        )
                                    }
                                    #[cfg(test)]
                                    V2IoCommand::RecoveredDecisionApplyFixture(_) => {
                                        unreachable!(
                                            "recovered Apply queue fixtures never enter a worker"
                                        )
                                    }
                                }
                            });
                            let failed = match completion {
                                Err(reason) => {
                                    if let Some(work_id) = work_id {
                                        command_rx.complete_work(work_id);
                                    }
                                    let _ = try_send_tracked_completion(
                                        &completion_tx,
                                        &worker_admission,
                                        V2IoCompletion::RecoveryRequired(reason.clone()),
                                    );
                                    true
                                }
                                Ok(completion) => {
                                    if let Some(work_id) = work_id {
                                        command_rx.complete_work(work_id);
                                    }
                                    let seal_result = match &completion {
                                        V2IoCompletion::RecoveredDecisionApply(guarded) => {
                                            recovered_decision_apply_key.map_or_else(
                                                || {
                                                    Err("recovered Decision Apply completion lost its command key"
                                                        .to_owned())
                                                },
                                                |key| {
                                                    command_rx
                                                        .complete_recovered_decision_apply(
                                                            key,
                                                            guarded.result(),
                                                        )
                                                        .map(|()| true)
                                                },
                                            )
                                        }
                                        V2IoCompletion::RecoveredLifecycleSign(guarded) => {
                                            recovered_lifecycle_sign_key.map_or_else(
                                                || {
                                                    Err("recovered Sign completion lost its command key"
                                                        .to_owned())
                                                },
                                                |key| {
                                                    command_rx
                                                        .complete_recovered_lifecycle_sign(
                                                            key,
                                                            guarded.result(),
                                                        )
                                                        .map(|()| true)
                                                },
                                            )
                                        }
                                        V2IoCompletion::RecoveredDecisionFetchBodyPersisted(
                                            guarded,
                                        ) => recovered_decision_fetch_key.map_or_else(
                                            || {
                                                Err("recovered Decision Fetch body completion lost its command key"
                                                    .to_owned())
                                            },
                                            |key| {
                                                command_rx
                                                    .complete_recovered_decision_fetch_body(
                                                        key,
                                                        guarded.completion(),
                                                    )
                                                    .map(|()| true)
                                            },
                                        ),
                                        V2IoCompletion::LifecycleCertifiedServe(guarded) => {
                                            command_rx
                                                .complete_lifecycle_certified_serve(
                                                    guarded.result(),
                                                )
                                                .map(|()| true)
                                        }
                                        _ => Ok(true),
                                    };
                                    match seal_result {
                                        Err(reason) => {
                                            iroha_logger::error!(
                                                %reason,
                                                "failed to seal Sumeragi v2 I/O completion"
                                            );
                                            let _ = try_send_tracked_completion(
                                                &completion_tx,
                                                &worker_admission,
                                                V2IoCompletion::RecoveryRequired(reason.clone()),
                                            );
                                            true
                                        }
                                        Ok(false) => {
                                            // A durable Decision installed
                                            // while this command was active.
                                            // The queue atomically published
                                            // the typed negative and released
                                            // admission, so no stale response
                                            // completion is exposed.
                                            false
                                        }
                                        Ok(true) => {
                                            send_completion_with_lifecycle_ordinal(
                                                &completion_tx,
                                                &worker_admission,
                                                Ok(completion),
                                                runtime_lifecycle_ordinal,
                                            );
                                            false
                                        }
                                    }
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
            V2IoTrySendError::Disconnected(_) => {
                "Sumeragi v2 I/O worker is disconnected".to_owned()
            }
            V2IoTrySendError::ConflictingWorkId { work_id, .. } => format!(
                "Sumeragi v2 I/O work {} was reused by a conflicting command",
                work_id.get()
            ),
            V2IoTrySendError::UnreservedRecoveredDecisionApply { .. } => {
                "recovered Decision Apply dispatch was reused by conflicting material".to_owned()
            }
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
    fn cancel(
        &self,
        work_id: EffectWorkId,
        expected_kind: V2IoCancellableKind,
    ) -> Result<bool, String> {
        self.command_tx.cancel(work_id, expected_kind)
    }
    fn acknowledge_completion_at(
        &self,
        acknowledgement: V2IoCompletionAcknowledgement,
        ownership_position: usize,
    ) -> Result<(), String> {
        match acknowledgement {
            V2IoCompletionAcknowledgement::Work(work_id) => {
                self.command_tx.acknowledge_completion(work_id);
            }
            V2IoCompletionAcknowledgement::LifecycleWorkRetained => {}
            V2IoCompletionAcknowledgement::RecoveredDecisionApplyRetained => {}
            V2IoCompletionAcknowledgement::RecoveredLifecycleSignRetained => {
                // Generic acknowledgement cannot perform the typed owner
                // settlement, so neither the command index nor its completion
                // owner may be removed here.
                return Ok(());
            }
            V2IoCompletionAcknowledgement::RecoveredDecisionFetchRetained => {
                return Ok(());
            }
            V2IoCompletionAcknowledgement::LifecycleServeRetained => return Ok(()),
            V2IoCompletionAcknowledgement::Untracked => {}
        }
        self.admission.acknowledge_completion_at(ownership_position);
        Ok(())
    }
    fn prepare_certified_fetch_body_persistence_ack(
        &self,
        completion: &CertifiedFetchBodyPersistenceCompletion,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<CertifiedFetchBodyPersistenceWorkAck, String> {
        self.command_tx
            .queue
            .prepare_certified_fetch_body_persistence_ack(completion, output_guard)
    }
    fn prepare_recovered_decision_apply_ack(
        &self,
        key: RecoveredDecisionApplyDispatchKeyV1,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<RecoveredDecisionApplyWorkAckV1, String> {
        self.command_tx
            .queue
            .prepare_recovered_decision_apply_ack(key, output_guard)
    }
    fn prepare_recovered_lifecycle_sign_completion(
        &self,
        guarded: Box<GuardedRecoveredLifecycleSignWorkerResultV1>,
        ownership_position: usize,
    ) -> Option<PreparedRecoveredLifecycleSignCompletionV1> {
        PreparedRecoveredLifecycleSignCompletionV1::new(
            guarded,
            Arc::clone(&self.command_tx.queue),
            ownership_position,
        )
    }
    fn prepare_recovered_decision_fetch_body_completion(
        &self,
        guarded: Box<GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1>,
        ownership_position: usize,
    ) -> Option<PreparedRecoveredDecisionFetchBodyCompletionV1> {
        PreparedRecoveredDecisionFetchBodyCompletionV1::new(
            guarded,
            Arc::clone(&self.command_tx.queue),
            ownership_position,
        )
    }
    fn prepare_lifecycle_certified_serve_completion(
        &self,
        guarded: Box<GuardedLifecycleCertifiedServeWorkerResultV1>,
        ownership_position: usize,
    ) -> Option<PreparedLifecycleCertifiedServeCompletionV1> {
        PreparedLifecycleCertifiedServeCompletionV1::new(
            guarded,
            Arc::clone(&self.command_tx.queue),
            ownership_position,
        )
    }
    fn acknowledge_completion(&self, completion: &V2IoCompletion) -> Result<(), String> {
        self.acknowledge_completion_at(completion.acknowledgement(), 0)
    }
    fn record_completion_service_attempt(&self, remaining_runtime_capacity: usize) -> bool {
        remaining_runtime_capacity == 0 && self.admission.record_completion_service_debt()
    }
    fn completion_snapshot(&self, now: Instant) -> RuntimeQueueLaneSnapshot {
        self.admission.completion_snapshot(now)
    }
    fn completion_requires_runtime_capacity_at(&self, position: usize) -> Option<bool> {
        self.admission
            .completion_requires_runtime_capacity_at(position)
    }
    fn completion_ownership_at(&self, position: usize) -> Option<V2IoCompletionOwnership> {
        self.admission.completion_ownership_at(position)
    }
    fn try_recv_completion_unacknowledged(&self) -> Result<V2IoCompletion, mpsc::TryRecvError> {
        self.completion_rx.try_recv()
    }
    #[cfg(test)]
    fn try_recv_completion(&self) -> Result<V2IoCompletion, mpsc::TryRecvError> {
        let completion = self.completion_rx.try_recv()?;
        self.acknowledge_completion(&completion)
            .expect("completion acknowledgement is infallible");
        Ok(completion)
    }
    fn recv_completion(&self) -> Result<V2IoCompletion, mpsc::RecvError> {
        let completion = self.completion_rx.recv()?;
        self.acknowledge_completion(&completion)
            .expect("completion acknowledgement is infallible");
        Ok(completion)
    }
    fn recv_completion_timeout(
        &self,
        timeout: Duration,
    ) -> Result<V2IoCompletion, mpsc::RecvTimeoutError> {
        let completion = self.completion_rx.recv_timeout(timeout)?;
        self.acknowledge_completion(&completion)
            .expect("completion acknowledgement is infallible");
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
                Err(V2IoTrySendError::Disconnected(_)) => break,
                Err(
                    V2IoTrySendError::ConflictingWorkId { .. }
                    | V2IoTrySendError::UnreservedRecoveredDecisionApply { .. },
                ) => {
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
    let recovered_decision_apply = completion.recovered_decision_apply_key();
    let recovered_lifecycle_sign = completion.recovered_lifecycle_sign_key();
    let recovered_decision_fetch = completion.recovered_decision_fetch_key();
    let lifecycle_certified_serve = completion.lifecycle_certified_serve_ordinal();
    admission.retain_completion(
        Instant::now(),
        completion.requires_runtime_capacity(),
        runtime_lifecycle_ordinal,
        recovered_decision_apply,
        recovered_lifecycle_sign,
        recovered_decision_fetch,
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
    let recovered_decision_apply = completion.recovered_decision_apply_key();
    let recovered_lifecycle_sign = completion.recovered_lifecycle_sign_key();
    let recovered_decision_fetch = completion.recovered_decision_fetch_key();
    let lifecycle_certified_serve = completion.lifecycle_certified_serve_ordinal();
    admission.retain_completion(
        Instant::now(),
        completion.requires_runtime_capacity(),
        runtime_lifecycle_ordinal,
        recovered_decision_apply,
        recovered_lifecycle_sign,
        recovered_decision_fetch,
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
impl LocalCompletion {
    const fn runtime_lifecycle_ordinal(&self) -> u128 {
        match self {
            Self::Reconstructed { task, .. } => task.lifecycle_ordinal(),
        }
    }
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
    pub(in crate::sumeragi) fn commit(
        self,
        permit: &ConsensusOutputPermit<'_>,
    ) -> CertifiedBodyFetchCompletionDisposition {
        assert!(
            permit.authorizes(self.services.output_guard.as_ref()),
            "certified body-fetch removal requires this service's live output permit"
        );
        self.services
            .commit_exact_body_fetch_owner_removal(&self.task, self.owner);
        CertifiedBodyFetchCompletionDisposition::Completed
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompletionSource {
    Io,
    Local,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompletionDrainPolicy {
    Fair,
    TimeoutRecoveryPrefix { inclusive_lifecycle_cut: u128 },
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactFanoutOwnership {
    /// Every post was admitted or the exact unadmitted suffix entered the corridor.
    Owned,
    /// The bounded corridor was full; the semantic producer must retain its source.
    SourceRetained,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExactOutputDriveOutcome {
    Drained,
    ReceiptBackpressured,
    Backpressured {
        closest_rank: usize,
    },
    BudgetExhausted {
        closest_backpressure_rank: Option<usize>,
    },
}
enum ExactOutputAttemptOutcome {
    Admitted,
    ReplyFlush(NetworkReplyFlushAck),
    SidecarFlush(NetworkReplyFlushAck),
    #[cfg(test)]
    TestReplyFlushed,
    Unavailable,
    Retired,
}
/// Process-local corridor/transport owner whose live endpoint identity binds a
/// handoff without entering wire, durable, or consensus state.
struct DurableExactOutputOwnerNonce {
    sealed: AtomicBool,
}
/// Exact-output endpoint retained by one [`ProductionV2Services`] instance.
pub(crate) struct DurableExactOutputServiceOwner(Arc<DurableExactOutputOwnerNonce>);
/// Paired endpoint retained beside one exact [`crate::merge_sidecar::MergeSidecarTransport`].
pub(crate) struct DurableExactOutputTransportOwner(Arc<DurableExactOutputOwnerNonce>);
/// Mint the unique service/transport owner pair for one height-local stack.
pub(crate) fn durable_exact_output_handoff_owner_pair() -> (
    DurableExactOutputServiceOwner,
    DurableExactOutputTransportOwner,
) {
    let owner = Arc::new(DurableExactOutputOwnerNonce {
        sealed: AtomicBool::new(false),
    });
    (
        DurableExactOutputServiceOwner(Arc::clone(&owner)),
        DurableExactOutputTransportOwner(owner),
    )
}
impl DurableExactOutputServiceOwner {
    /// Return whether this service endpoint was minted with one transport endpoint.
    pub(in crate::sumeragi) fn is_bound_to_transport_owner(
        &self,
        owner: &DurableExactOutputTransportOwner,
    ) -> bool {
        Arc::ptr_eq(&self.0, &owner.0)
    }
    fn is_sealed(&self) -> bool {
        self.0.sealed.load(AtomicOrdering::Acquire)
    }
    fn seal(&self) -> Result<(), String> {
        self.0
            .sealed
            .compare_exchange(false, true, AtomicOrdering::AcqRel, AtomicOrdering::Acquire)
            .map(|_| ())
            .map_err(|_| "Sumeragi v2 durable exact-output handoff was already sealed".to_owned())
    }
}
#[cfg(test)]
impl DurableExactOutputTransportOwner {
    /// Reconstruct the paired test endpoint without exposing the owner nonce.
    pub(in crate::sumeragi) fn paired_service_for_test(&self) -> DurableExactOutputServiceOwner {
        DurableExactOutputServiceOwner(Arc::clone(&self.0))
    }
}
/// Move-only durable-supersession proof binding canonical hashes to the private
/// process-local service endpoint, excluding independently created services.
#[must_use]
pub(crate) struct DurableExactOutputHandoffReceipt {
    owner: Arc<DurableExactOutputOwnerNonce>,
    predecessor_context_hash: HashOf<wire::HeightContext>,
    predecessor_context_id: wire::HeightContextId,
    predecessor_height: u64,
    predecessor_network_id: iroha_data_model::NetworkId,
    finality_artifact_hash: HashOf<wire::finality::V2FinalityArtifact>,
    finality_commit_qc: wire::QuorumCertificate,
}
impl DurableExactOutputHandoffReceipt {
    /// Return whether this receipt names the transport endpoint paired with its service.
    pub(crate) fn is_bound_to_transport_owner(
        &self,
        owner: &DurableExactOutputTransportOwner,
    ) -> bool {
        Arc::ptr_eq(&self.owner, &owner.0)
    }
    /// Match the receipt's full canonical predecessor context identity.
    pub(crate) fn matches_predecessor_context(&self, context: &wire::HeightContext) -> bool {
        self.predecessor_context_hash == HashOf::new(context)
            && self.predecessor_context_id == context.id()
            && self.predecessor_height == context.height
            && self.predecessor_network_id == context.network_id
    }
    /// Match the exact durable finality artifact that authorized the seal.
    pub(crate) fn matches_finality_artifact(
        &self,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> bool {
        self.finality_artifact_hash == HashOf::new(artifact)
            && self.predecessor_context_hash == HashOf::new(&artifact.height_context)
            && self.predecessor_context_id == artifact.context_id()
            && self.predecessor_height == artifact.height
            && self.predecessor_network_id == artifact.height_context.network_id
            && self.finality_commit_qc == artifact.commit_qc
    }
    /// Verify the exact parent QC and height relation for one immediate successor.
    pub(crate) fn authorizes_immediate_successor(&self, successor: &wire::HeightContext) -> bool {
        self.predecessor_height.checked_add(1) == Some(successor.height)
            && self.predecessor_network_id == successor.network_id
            && successor.parent_commit_qc.as_ref() == Some(&self.finality_commit_qc)
            && self.finality_commit_qc.round.context_id == self.predecessor_context_id
            && self.finality_commit_qc.round.height == self.predecessor_height
    }
}
fn certified_sidecar_prefix_covers_occurrence(
    prefix: &CertifiedMergeSidecarClosedPrefix,
    requester: &PeerId,
    service_generation: crate::merge_sidecar::CertifiedMergeSidecarServiceGenerationV1,
    stream_epoch: CertifiedMergeSidecarStreamEpochV1,
    semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1,
) -> bool {
    requester == &prefix.requester
        && (service_generation < prefix.service_generation
            || (service_generation == prefix.service_generation
                && (stream_epoch < prefix.stream_epoch
                    || (stream_epoch == prefix.stream_epoch
                        && semantic_sequence.get() <= prefix.closed_through))))
}
/// Bounded per-target FIFO owner for semantic network output awaiting actor admission.
#[derive(Debug)]
struct PendingExactOutput {
    fanouts: VecDeque<PendingExactFanout>,
    /// Writer-flushed sidecar cursor receipts not yet applied by lane work.
    admitted_sidecar_chunks: VecDeque<CertifiedMergeSidecarChunkAdmission>,
    /// Separate byte-free control-queue bound for sidecar admission receipts.
    sidecar_admission_capacity: usize,
    next_fanout_index: usize,
    /// Next stable enqueue sequence between deterministic overflow rebases.
    next_fanout_fifo_id: ExactFanoutFifoId,
    /// Every outstanding authenticated source mapped to its FIFO-ordered owners.
    source_fifo_owners: BTreeMap<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>,
    /// Ownership-unit bound: shared units, one unit for every frozen
    /// target/class pair, one pacemaker unit, one sidecar topology-progress
    /// unit, and one reproducible exact-reply control unit per frozen target.
    ownership_unit_capacity: usize,
    /// Units available to duplicate or non-frozen target/class ownership.
    shared_ownership_unit_capacity: usize,
    /// Per-target reliable, pacemaker, topology-progress, and reply-control
    /// reservation geometry frozen for this height.
    reserved_target_classes: BTreeSet<ExactTargetReservation>,
    /// Aggregate outstanding multiplicity for each semantic target/class/kind unit.
    reservation_owner_counts: BTreeMap<ExactTargetReservation, usize>,
    /// Total outstanding target/class/kind ownership units in retained fanouts.
    ownership_units: usize,
    /// Outstanding units not covered by the first frozen target/class/kind credit.
    shared_ownership_units: usize,
    /// Deterministic actor-admission attempts before yielding to the runner.
    ///
    /// Atomic Proposal admission retains the two pre-atomic child slices: one
    /// for control and one for chunks.
    drive_attempt_budget: usize,
    max_messages_per_fanout: usize,
    max_peers_per_fanout: usize,
}
/// Precomputed topology-batch mutation held under one mutex after all fallible
/// validation, capacity, FIFO, and index projection.
struct PendingExactOutputBatchPlan {
    existing_fanout_count: usize,
    rebased_existing_fifo_ids: Option<Vec<ExactFanoutFifoId>>,
    fanouts: Vec<PendingExactFanout>,
    source_fifo_owners: BTreeMap<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>,
    reservation_owner_counts: BTreeMap<ExactTargetReservation, usize>,
    ownership_units: usize,
    shared_ownership_units: usize,
    next_fanout_fifo_id: ExactFanoutFifoId,
}
impl PendingExactOutput {
    fn new(
        shared_ownership_unit_capacity: usize,
        max_messages_per_fanout: usize,
        max_peers_per_fanout: usize,
        frozen_semantic_targets: &[PeerId],
    ) -> Result<Self, String> {
        if shared_ownership_unit_capacity == 0
            || max_messages_per_fanout == 0
            || max_peers_per_fanout == 0
        {
            return Err("Sumeragi v2 outbound corridor bounds must be non-zero".to_owned());
        }
        let reserved_target_classes = frozen_semantic_targets
            .iter()
            .flat_map(|semantic_target| {
                EXACT_OUTPUT_CLASSES
                    .map(|class| ExactTargetReservation {
                        semantic_target: semantic_target.clone(),
                        class,
                        kind: ExactTargetReservationKind::Reliable,
                    })
                    .into_iter()
                    .chain([ExactTargetReservation {
                        semantic_target: semantic_target.clone(),
                        class: ExactOutputClass::Safety,
                        kind: ExactTargetReservationKind::Pacemaker,
                    }])
                    .chain([ExactTargetReservation {
                        semantic_target: semantic_target.clone(),
                        // Topology-routed Request/Close progress is canonical
                        // Consensus traffic and therefore uses the Lane class.
                        class: ExactOutputClass::Lane,
                        kind: ExactTargetReservationKind::SidecarTopologyProgress,
                    }])
                    .chain([ExactTargetReservation {
                        semantic_target: semantic_target.clone(),
                        // Stateless responder controls retain exact reply
                        // authority but cannot be starved by ordinary Lane
                        // output for the same semantic target.
                        class: ExactOutputClass::Lane,
                        kind: ExactTargetReservationKind::SidecarReplyControl,
                    }])
            })
            .collect::<BTreeSet<_>>();
        let sidecar_admission_capacity = shared_ownership_unit_capacity
            .checked_add(
                reserved_target_classes
                    .iter()
                    .filter(|reservation| reservation.kind == ExactTargetReservationKind::Reliable)
                    .count(),
            )
            .ok_or_else(|| "Sumeragi v2 sidecar admission capacity overflowed".to_owned())?;
        let ownership_unit_capacity = shared_ownership_unit_capacity
            .checked_add(reserved_target_classes.len())
            .ok_or_else(|| "Sumeragi v2 outbound corridor capacity overflowed".to_owned())?;
        let drive_attempt_budget = max_peers_per_fanout
            .max(super::v2_core::MAX_EFFECTS_PER_STEP)
            .checked_mul(ATOMIC_PROPOSAL_FANOUT_COUNT)
            .ok_or_else(|| "Sumeragi v2 outbound drive budget overflowed".to_owned())?;
        Ok(Self {
            fanouts: VecDeque::new(),
            admitted_sidecar_chunks: VecDeque::new(),
            sidecar_admission_capacity,
            next_fanout_index: 0,
            next_fanout_fifo_id: 0,
            source_fifo_owners: BTreeMap::new(),
            ownership_unit_capacity,
            shared_ownership_unit_capacity,
            reserved_target_classes,
            reservation_owner_counts: BTreeMap::new(),
            ownership_units: 0,
            shared_ownership_units: 0,
            drive_attempt_budget,
            max_messages_per_fanout,
            max_peers_per_fanout,
        })
    }
    /// Preflight an all-or-nothing fresh topology batch, aggregating Proposal
    /// control/chunk multiplicities once and excluding stateful replacements.
    #[allow(clippy::too_many_lines)]
    fn prepare_atomic_fanout_batch(
        &self,
        mut fanouts: Vec<PendingExactFanout>,
    ) -> Result<Option<PendingExactOutputBatchPlan>, String> {
        let existing_fanout_count = self.fanouts.len();
        let mut additions = BTreeMap::<ExactTargetReservation, usize>::new();
        for fanout in &fanouts {
            self.validate_fanout_bounds(fanout)?;
            if fanout.is_complete()
                || fanout.reply_routes.is_some()
                || fanout.ingress_ownership.is_some()
                || fanout
                    .targets
                    .iter()
                    .any(|target| !matches!(target.route, ExactTargetRoute::Topology))
                || self
                    .fanouts
                    .iter()
                    .any(|retained| retained.can_coalesce_retry(fanout))
                || self
                    .stranded_responder_control_replacement_index(fanout)
                    .is_some()
                || self.retains_retryable_sidecar_responder_control_for(fanout)
            {
                return Err(
                    "Sumeragi v2 atomic Proposal output changed fresh topology geometry".to_owned(),
                );
            }
            for (reservation, count) in fanout.outstanding_reservation_counts()? {
                let aggregate = additions.entry(reservation).or_default();
                *aggregate = aggregate.checked_add(count).ok_or_else(|| {
                    "Sumeragi v2 atomic Proposal output ownership overflowed".to_owned()
                })?;
            }
        }
        if !self.ownership_capacity_available(&additions)? {
            return Ok(None);
        }
        let (reservation_owner_counts, ownership_units, shared_ownership_units) =
            self.ownership_state_after_additions(&additions)?;
        let project_ids = |first: ExactFanoutFifoId| {
            let mut cursor = first;
            let mut ids = Vec::with_capacity(fanouts.len());
            for _ in &fanouts {
                if cursor == ExactFanoutFifoId::MAX {
                    return None;
                }
                ids.push(cursor);
                cursor = cursor.checked_add(1)?;
            }
            Some((ids, cursor))
        };
        let (
            rebased_existing_fifo_ids,
            fanout_fifo_ids,
            next_fanout_fifo_id,
            mut source_fifo_owners,
        ) = if let Some((ids, next)) = project_ids(self.next_fanout_fifo_id) {
            (None, ids, next, self.source_fifo_owners.clone())
        } else {
            let mut rebuilt = BTreeMap::<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>::new();
            let mut existing_ids = Vec::with_capacity(self.fanouts.len());
            for (index, retained) in self.fanouts.iter().enumerate() {
                let fifo_id = ExactFanoutFifoId::try_from(index).map_err(|_| {
                    "Sumeragi v2 atomic Proposal FIFO rebase is not representable".to_owned()
                })?;
                existing_ids.push(fifo_id);
                for source in retained.outstanding_sources()? {
                    rebuilt.entry(source).or_default().insert(fifo_id);
                }
            }
            let first = ExactFanoutFifoId::try_from(self.fanouts.len()).map_err(|_| {
                "Sumeragi v2 atomic Proposal FIFO sequence is not representable".to_owned()
            })?;
            let (ids, next) = project_ids(first)
                .ok_or_else(|| "Sumeragi v2 atomic Proposal FIFO sequence exhausted".to_owned())?;
            (Some(existing_ids), ids, next, rebuilt)
        };
        if fanout_fifo_ids.iter().any(|fifo_id| {
            source_fifo_owners
                .values()
                .any(|owners| owners.contains(fifo_id))
        }) {
            return Err("Sumeragi v2 atomic Proposal FIFO reused a live identity".to_owned());
        }
        for (fanout, fifo_id) in fanouts.iter_mut().zip(fanout_fifo_ids) {
            for source in fanout.outstanding_sources()? {
                source_fifo_owners
                    .entry(source)
                    .or_default()
                    .insert(fifo_id);
            }
            fanout.fifo_id = Some(fifo_id);
        }
        Ok(Some(PendingExactOutputBatchPlan {
            existing_fanout_count,
            rebased_existing_fifo_ids,
            fanouts,
            source_fifo_owners,
            reservation_owner_counts,
            ownership_units,
            shared_ownership_units,
            next_fanout_fifo_id,
        }))
    }
    /// Commit a batch prepared while this exact mutex guard remained held.
    fn commit_atomic_fanout_batch(&mut self, plan: PendingExactOutputBatchPlan) {
        let PendingExactOutputBatchPlan {
            existing_fanout_count,
            rebased_existing_fifo_ids,
            fanouts,
            source_fifo_owners,
            reservation_owner_counts,
            ownership_units,
            shared_ownership_units,
            next_fanout_fifo_id,
        } = plan;
        assert_eq!(
            self.fanouts.len(),
            existing_fanout_count,
            "atomic Proposal output retained the corridor mutex"
        );
        if let Some(rebased) = rebased_existing_fifo_ids {
            assert_eq!(rebased.len(), self.fanouts.len());
            for (fanout, fifo_id) in self.fanouts.iter_mut().zip(rebased) {
                fanout.fifo_id = Some(fifo_id);
            }
        }
        self.fanouts.extend(fanouts);
        self.source_fifo_owners = source_fifo_owners;
        self.reservation_owner_counts = reservation_owner_counts;
        self.ownership_units = ownership_units;
        self.shared_ownership_units = shared_ownership_units;
        self.next_fanout_fifo_id = next_fanout_fifo_id;
    }
    fn is_pending(&self) -> bool {
        self.fanouts.iter().any(|fanout| {
            fanout.has_dispatchable_target()
                || fanout
                    .targets
                    .iter()
                    .any(|target| target.pending_flush.is_some())
        }) || !self.admitted_sidecar_chunks.is_empty()
    }
    fn pending_kura_replica_advert_heights(&self) -> Result<BTreeSet<u64>, String> {
        let mut heights = BTreeSet::new();
        for fanout in &self.fanouts {
            let ExactOutputRolloverClaim::DurableKuraReplicaAdvert { source_height, .. } =
                &fanout.rollover_claim
            else {
                continue;
            };
            fanout
                .rollover_claim
                .validate_fanout(&fanout.messages, &fanout.semantic_peers())?;
            if *source_height == 0 {
                return Err(
                    "pending Kura replica advert lost its non-zero durable source height"
                        .to_owned(),
                );
            }
            heights.insert(*source_height);
        }
        Ok(heights)
    }
    fn remove_fanouts_matching(
        &mut self,
        covered: impl Fn(&PendingExactFanout) -> bool,
        validate_removed: impl Fn(&PendingExactFanout) -> Result<(), String>,
        operation: &'static str,
    ) -> Result<usize, String> {
        let mut current_sources = BTreeMap::<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>::new();
        let mut current_reservations = BTreeMap::<ExactTargetReservation, usize>::new();
        let mut retained_sources =
            BTreeMap::<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>::new();
        let mut retained_reservations = BTreeMap::<ExactTargetReservation, usize>::new();
        let mut removed = 0usize;
        for fanout in &self.fanouts {
            if fanout.message_hashes.len() != fanout.messages.len()
                || fanout
                    .messages
                    .iter()
                    .zip(&fanout.message_hashes)
                    .any(|(message, expected)| HashOf::new(message) != *expected)
            {
                return Err(format!(
                    "Sumeragi v2 {operation} found altered exact-output payload"
                ));
            }
            let fifo_id = fanout.fifo_id.ok_or_else(|| {
                format!("Sumeragi v2 {operation} found an unowned exact-output fanout")
            })?;
            let sources = fanout.outstanding_sources()?;
            let reservations = fanout.outstanding_reservation_counts()?;
            for source in &sources {
                current_sources
                    .entry(source.clone())
                    .or_default()
                    .insert(fifo_id);
            }
            for (reservation, count) in &reservations {
                let aggregate = current_reservations.entry(reservation.clone()).or_default();
                *aggregate = aggregate
                    .checked_add(*count)
                    .ok_or_else(|| format!("Sumeragi v2 {operation} ownership count overflowed"))?;
            }
            if covered(fanout) {
                validate_removed(fanout)?;
                removed = removed
                    .checked_add(1)
                    .ok_or_else(|| format!("Sumeragi v2 {operation} count overflowed"))?;
                continue;
            }
            for source in sources {
                retained_sources.entry(source).or_default().insert(fifo_id);
            }
            for (reservation, count) in reservations {
                let aggregate = retained_reservations.entry(reservation).or_default();
                *aggregate = aggregate.checked_add(count).ok_or_else(|| {
                    format!("Sumeragi v2 retained {operation} ownership count overflowed")
                })?;
            }
        }
        if current_sources != self.source_fifo_owners
            || current_reservations != self.reservation_owner_counts
        {
            return Err(format!(
                "Sumeragi v2 {operation} found inconsistent exact-output ownership"
            ));
        }
        let mut retained_units = 0usize;
        let mut retained_shared_units = 0usize;
        for (reservation, count) in &retained_reservations {
            retained_units = retained_units
                .checked_add(*count)
                .ok_or_else(|| format!("Sumeragi v2 retained {operation} units overflowed"))?;
            let frozen_credit = usize::from(self.reserved_target_classes.contains(reservation));
            retained_shared_units = retained_shared_units
                .checked_add(count.checked_sub(frozen_credit).ok_or_else(|| {
                    format!("Sumeragi v2 retained {operation} frozen credit exceeded ownership")
                })?)
                .ok_or_else(|| {
                    format!("Sumeragi v2 retained {operation} shared units overflowed")
                })?;
        }
        self.fanouts.retain(|fanout| !covered(fanout));
        self.source_fifo_owners = retained_sources;
        self.reservation_owner_counts = retained_reservations;
        self.ownership_units = retained_units;
        self.shared_ownership_units = retained_shared_units;
        self.next_fanout_index = if self.fanouts.is_empty() {
            0
        } else {
            self.next_fanout_index % self.fanouts.len()
        };
        Ok(removed)
    }
    fn close_certified_sidecar_prefix(
        &mut self,
        prefix: &CertifiedMergeSidecarClosedPrefix,
    ) -> Result<usize, String> {
        let covered = |fanout: &PendingExactFanout| {
            matches!(
                &fanout.rollover_claim,
                ExactOutputRolloverClaim::CertifiedSidecarChunk { transfer, .. }
                    if certified_sidecar_prefix_covers_occurrence(
                        prefix,
                        &transfer.requester,
                        transfer.service_generation,
                        transfer.stream_epoch,
                        transfer.semantic_sequence,
                )
            )
        };
        let removed = self.remove_fanouts_matching(
            covered,
            |fanout| {
                fanout
                    .is_certified_sidecar_chunk_fanout()
                    .then_some(())
                    .ok_or_else(|| {
                        "Sumeragi v2 sidecar close claim covers a different output kind".to_owned()
                    })
            },
            "sidecar close",
        )?;
        self.admitted_sidecar_chunks.retain(|admission| {
            let projection = admission.projection();
            !certified_sidecar_prefix_covers_occurrence(
                prefix,
                &projection.requester,
                projection.service_generation,
                projection.stream_epoch,
                projection.semantic_sequence,
            )
        });
        debug_assert!(self.sidecar_control_units() <= self.sidecar_admission_capacity);
        Ok(removed)
    }
    fn cancel_historical_lane_recovery_requests(
        &mut self,
        request_hashes: &BTreeSet<HashOf<LaneHistoricalRecoveryRequestV1>>,
    ) -> Result<usize, String> {
        if request_hashes.is_empty() {
            return Ok(0);
        }
        self.remove_fanouts_matching(
            |fanout| {
                matches!(
                    &fanout.rollover_claim,
                    ExactOutputRolloverClaim::HistoricalLaneRecoveryRequest {
                        request_hash,
                        ..
                    } if request_hashes.contains(request_hash)
                )
            },
            |fanout| {
                fanout
                    .rollover_claim
                    .validate_fanout(&fanout.messages, &fanout.semantic_peers())
            },
            "historical recovery cancellation",
        )
    }
    fn cancel_certified_merge_sidecar_requests(
        &mut self,
        request_hashes: &BTreeSet<HashOf<CertifiedMergeSidecarRequestV1>>,
    ) -> Result<usize, String> {
        if request_hashes.is_empty() {
            return Ok(0);
        }
        self.remove_fanouts_matching(
            |fanout| {
                matches!(
                    &fanout.rollover_claim,
                    ExactOutputRolloverClaim::CertifiedSidecarRequest {
                        request_hash,
                        ..
                    } if request_hashes.contains(request_hash)
                )
            },
            |fanout| {
                fanout
                    .rollover_claim
                    .validate_fanout(&fanout.messages, &fanout.semantic_peers())
            },
            "certified merge-sidecar request cancellation",
        )
    }
    fn cancel_acknowledged_certified_merge_sidecar_closes(
        &mut self,
        acknowledgements: &[CertifiedMergeSidecarCloseAckV1],
    ) -> Result<usize, String> {
        if acknowledgements.is_empty() {
            return Ok(0);
        }
        if acknowledgements.iter().any(|acknowledgement| {
            acknowledgement.version != CERTIFIED_MERGE_SIDECAR_VERSION_V1
                || acknowledgement.closed_through == 0
                || acknowledgement.close_id != acknowledgement.canonical_close_id()
        }) {
            return Err(
                "Sumeragi v2 requester Close cancellation has an invalid acknowledgement prefix"
                    .to_owned(),
            );
        }
        self.remove_fanouts_matching(
            |fanout| {
                matches!(
                    fanout.messages.as_slice(),
                    [NetworkMessage::CertifiedMergeSidecar(message)]
                        if matches!(
                            message.as_ref(),
                            CertifiedMergeSidecarMessage::Close(close)
                                if acknowledgements.iter().any(|acknowledgement| {
                                    acknowledgement.covers_requester_close(close)
                                })
                        )
                )
            },
            |fanout| {
                fanout
                    .rollover_claim
                    .validate_fanout(&fanout.messages, &fanout.semantic_peers())
            },
            "acknowledged certified merge-sidecar Close cancellation",
        )
    }
    fn pending_sidecar_flushes(&self) -> usize {
        self.fanouts
            .iter()
            .flat_map(|fanout| &fanout.targets)
            .filter(|target| {
                target
                    .pending_flush
                    .as_ref()
                    .is_some_and(|pending| pending.sidecar_admission.is_some())
            })
            .count()
    }
    fn sidecar_control_units(&self) -> usize {
        self.pending_sidecar_flushes()
            .saturating_add(self.admitted_sidecar_chunks.len())
    }
    fn restore_pending_flush(
        &mut self,
        fanout_index: usize,
        target_index: usize,
        pending_flush: PendingExactReplyFlush,
    ) -> Result<(), String> {
        let target = self
            .fanouts
            .get_mut(fanout_index)
            .and_then(|fanout| fanout.targets.get_mut(target_index))
            .ok_or_else(|| {
                "Sumeragi v2 reply flush target disappeared during validation".to_owned()
            })?;
        if target.pending_flush.replace(pending_flush).is_some() {
            return Err("Sumeragi v2 reply target acquired two writer flushes".to_owned());
        }
        Ok(())
    }
    fn poll_reply_flushes(&mut self) -> Result<(), String> {
        loop {
            let mut terminal = None;
            'scan: for (fanout_index, fanout) in self.fanouts.iter_mut().enumerate() {
                for (target_index, target) in fanout.targets.iter_mut().enumerate() {
                    let Some(pending_flush) = target.pending_flush.as_mut() else {
                        continue;
                    };
                    let status = pending_flush.flush_ack.poll();
                    if !matches!(status, NetworkReplyFlushAckStatus::Pending) {
                        terminal = Some((fanout_index, target_index, status));
                        break 'scan;
                    }
                }
            }
            let Some((fanout_index, target_index, status)) = terminal else {
                return Ok(());
            };
            let (
                canonical_post,
                attempted_source,
                current_route,
                current_timeout_attempt,
                was_parked,
            ) = {
                let fanout = self
                    .fanouts
                    .get(fanout_index)
                    .ok_or_else(|| "Sumeragi v2 flushing reply fanout disappeared".to_owned())?;
                let target = fanout
                    .targets
                    .get(target_index)
                    .ok_or_else(|| "Sumeragi v2 flushing reply target disappeared".to_owned())?;
                let ExactTargetRoute::Reply(route) = &target.route else {
                    return Err("Sumeragi v2 topology target retained a reply flush".to_owned());
                };
                let data = fanout
                    .messages
                    .get(target.message_index)
                    .ok_or_else(|| {
                        "Sumeragi v2 reply flush advanced beyond its immutable payload".to_owned()
                    })?
                    .clone();
                let peer_id = fanout
                    .peers
                    .get(target_index)
                    .ok_or_else(|| "Sumeragi v2 reply flush lost its target".to_owned())?
                    .clone();
                let class = exact_output_class(&data)?;
                (
                    Post {
                        data,
                        peer_id: peer_id.clone(),
                        priority: Priority::High,
                    },
                    target.route.source(&peer_id, class),
                    route.clone(),
                    target.reply_writer_timeout_attempt,
                    target.parked,
                )
            };
            let sidecar_flushing_before = self.pending_sidecar_flushes();
            let checked_flush_trace = {
                let pending_flush = self
                    .fanouts
                    .get(fanout_index)
                    .and_then(|fanout| fanout.targets.get(target_index))
                    .and_then(|target| target.pending_flush.as_ref())
                    .ok_or_else(|| "Sumeragi v2 terminal reply flush lost ownership".to_owned())?;
                if !pending_flush
                    .flush_ack
                    .identity()
                    .is_bound_to_canonical_reply(&canonical_post)
                    || pending_flush.flush_ack.identity().source_key() != current_route.source_key()
                    || pending_flush.reply_writer_timeout_attempt != current_timeout_attempt
                    || pending_flush
                        .flush_ack
                        .identity()
                        .reply_writer_timeout_attempt()
                        != pending_flush.reply_writer_timeout_attempt
                {
                    return Err(
                        "Sumeragi v2 terminal reply flush changed payload, source, or timeout-attempt identity"
                            .to_owned(),
                    );
                }
                if let Some(admission) = pending_flush.sidecar_admission.as_ref() {
                    if !admission.matches_ack_identity(pending_flush.flush_ack.identity()) {
                        return Err(
                            MergeSidecarError::FlushIdentityMismatch(
                                "queued admission and writer acknowledgement identify different actor output",
                            )
                            .to_string(),
                        );
                    }
                    let flushing_before = u64::try_from(sidecar_flushing_before)
                        .expect("bounded sidecar flush count is representable as u64");
                    let flushing_after = flushing_before.checked_sub(1).ok_or_else(|| {
                        MergeSidecarError::FlushIdentityMismatch(
                            "sidecar flushing-owner count underflowed",
                        )
                        .to_string()
                    })?;
                    let admitted_before = u64::try_from(self.admitted_sidecar_chunks.len())
                        .expect("bounded sidecar admission count is representable as u64");
                    let admitted_after = if matches!(status, NetworkReplyFlushAckStatus::Flushed) {
                        admitted_before.checked_add(1).ok_or_else(|| {
                            MergeSidecarError::FlushIdentityMismatch(
                                "sidecar admitted-owner count overflowed",
                            )
                            .to_string()
                        })?
                    } else {
                        admitted_before
                    };
                    let flush_trace = reliable_flush_trace_projection(
                        admission,
                        status,
                        flushing_before,
                        flushing_after,
                        admitted_before,
                        admitted_after,
                        self.sidecar_admission_capacity,
                    )
                    .map_err(|error| error.to_string())?;
                    Some(
                        check_production_reliable_flush_worker_transition(flush_trace)
                            .ok_or_else(|| {
                                MergeSidecarError::FlushIdentityMismatch(
                                    "sidecar flush transition failed its exact ownership kernel",
                                )
                                .to_string()
                            })?
                            .into_projection(),
                    )
                } else {
                    None
                }
            };
            let mut pending_flush = self
                .fanouts
                .get_mut(fanout_index)
                .and_then(|fanout| fanout.targets.get_mut(target_index))
                .and_then(|target| target.pending_flush.take())
                .ok_or_else(|| "Sumeragi v2 terminal reply flush lost ownership".to_owned())?;
            if let Some(admission) = pending_flush.sidecar_admission.as_mut() {
                let flush_trace = checked_flush_trace.ok_or_else(|| {
                    MergeSidecarError::FlushIdentityMismatch(
                        "sidecar admission lost its pre-authorized worker transition",
                    )
                    .to_string()
                })?;
                if matches!(status, NetworkReplyFlushAckStatus::Flushed)
                    && let Err(error) = admission.bind_confirmed_worker_trace(flush_trace)
                {
                    let error = error.to_string();
                    self.restore_pending_flush(fanout_index, target_index, pending_flush)?;
                    return Err(error);
                }
            }
            match status {
                NetworkReplyFlushAckStatus::Pending => {
                    unreachable!("terminal scan excludes pending")
                }
                NetworkReplyFlushAckStatus::Flushed => {
                    if pending_flush.sidecar_admission.is_none()
                        && !pending_flush.flush_ack.identity().claim_writer_flush_once()
                    {
                        self.restore_pending_flush(fanout_index, target_index, pending_flush)?;
                        return Err(
                            "Sumeragi v2 reply writer flush was consumed more than once".to_owned()
                        );
                    }
                    if was_parked {
                        self.fanouts
                            .get_mut(fanout_index)
                            .and_then(|fanout| fanout.targets.get_mut(target_index))
                            .expect("flushing parked target must remain present")
                            .parked = false;
                    }
                    self.fanouts
                        .get_mut(fanout_index)
                        .expect("flushed reply fanout must remain present")
                        .mark_admitted(target_index)?;
                    if was_parked {
                        let fanout = self
                            .fanouts
                            .get_mut(fanout_index)
                            .expect("flushed parked fanout must remain present");
                        let target_complete = fanout.target_is_complete(target_index);
                        let target = fanout
                            .targets
                            .get_mut(target_index)
                            .expect("flushed parked target must remain present");
                        let writable = matches!(&target.route, ExactTargetRoute::Topology)
                            || matches!(&target.route,
                                ExactTargetRoute::Reply(route) if route.is_reply_writable());
                        if !target_complete && !writable {
                            target.parked = true;
                        }
                    }
                    self.advance_after_attempt(
                        fanout_index,
                        target_index,
                        Some(&attempted_source),
                    )?;
                    if let Some(admission) = pending_flush.sidecar_admission.take() {
                        self.admitted_sidecar_chunks.push_back(admission);
                    }
                }
                NetworkReplyFlushAckStatus::TimedOut | NetworkReplyFlushAckStatus::Closed => {
                    if matches!(status, NetworkReplyFlushAckStatus::TimedOut) {
                        let target = self
                            .fanouts
                            .get_mut(fanout_index)
                            .and_then(|fanout| fanout.targets.get_mut(target_index))
                            .ok_or_else(|| {
                                "Sumeragi v2 timed-out reply flush lost its target".to_owned()
                            })?;
                        target.reply_writer_timeout_attempt =
                            target.reply_writer_timeout_attempt.saturating_add(1);
                    }
                    let route_state = self
                        .fanouts
                        .get(fanout_index)
                        .and_then(|fanout| fanout.targets.get(target_index))
                        .and_then(|target| match &target.route {
                            ExactTargetRoute::Reply(route) => {
                                Some((route.is_active(), route.is_reply_writable(), target.parked))
                            }
                            ExactTargetRoute::Topology => None,
                        })
                        .ok_or_else(|| {
                            "Sumeragi v2 terminal reply flush lost its route".to_owned()
                        })?;
                    if !route_state.2 {
                        if !route_state.0 {
                            self.retire_inactive_reply_target(fanout_index, target_index)?;
                        } else if !route_state.1 {
                            self.park_unwritable_reply_target(fanout_index, target_index)?;
                        }
                    }
                }
            }
            debug_assert!(self.sidecar_control_units() <= self.sidecar_admission_capacity);
        }
    }
    fn rebase_source_fifo(&mut self) -> Result<(), String> {
        let mut rebuilt = BTreeMap::<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>::new();
        let mut rebased_ids = Vec::with_capacity(self.fanouts.len());
        for (fanout_index, fanout) in self.fanouts.iter().enumerate() {
            let fifo_id = ExactFanoutFifoId::try_from(fanout_index)
                .map_err(|_| "Sumeragi v2 outbound FIFO index is not representable".to_owned())?;
            rebased_ids.push(fifo_id);
            for source in fanout.outstanding_sources()? {
                rebuilt.entry(source).or_default().insert(fifo_id);
            }
        }
        let next_fanout_fifo_id = ExactFanoutFifoId::try_from(self.fanouts.len())
            .map_err(|_| "Sumeragi v2 outbound FIFO sequence is not representable".to_owned())?;
        if next_fanout_fifo_id == ExactFanoutFifoId::MAX {
            return Err("Sumeragi v2 outbound FIFO sequence exhausted".to_owned());
        }
        for (fanout, fifo_id) in self.fanouts.iter_mut().zip(rebased_ids) {
            fanout.fifo_id = Some(fifo_id);
        }
        self.next_fanout_fifo_id = next_fanout_fifo_id;
        self.source_fifo_owners = rebuilt;
        Ok(())
    }
    fn allocate_fanout_fifo_id(&mut self) -> Result<ExactFanoutFifoId, String> {
        if self.next_fanout_fifo_id == ExactFanoutFifoId::MAX {
            self.rebase_source_fifo()?;
        }
        let fifo_id = self.next_fanout_fifo_id;
        if self
            .source_fifo_owners
            .values()
            .any(|owners| owners.contains(&fifo_id))
        {
            return Err("Sumeragi v2 outbound FIFO sequence reused a live identity".to_owned());
        }
        self.next_fanout_fifo_id = fifo_id
            .checked_add(1)
            .ok_or_else(|| "Sumeragi v2 outbound FIFO sequence exhausted".to_owned())?;
        Ok(fifo_id)
    }
    fn unregister_source_fifo_owner(
        &mut self,
        fifo_id: ExactFanoutFifoId,
        source: &ExactTargetSource,
    ) -> Result<(), String> {
        let remove_source = {
            let owners = self
                .source_fifo_owners
                .get_mut(source)
                .ok_or_else(|| "Sumeragi v2 outbound FIFO lost a registered source".to_owned())?;
            if !owners.remove(&fifo_id) {
                return Err("Sumeragi v2 outbound FIFO lost a registered owner".to_owned());
            }
            owners.is_empty()
        };
        if remove_source {
            self.source_fifo_owners.remove(source);
        }
        Ok(())
    }
    fn source_fifo_owners_after_fanout_replacement(
        &self,
        fifo_id: ExactFanoutFifoId,
        prior_sources: &BTreeSet<ExactTargetSource>,
        updated_sources: &BTreeSet<ExactTargetSource>,
    ) -> Result<BTreeMap<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>, String> {
        let indexed_sources = self
            .source_fifo_owners
            .iter()
            .filter_map(|(source, owners)| owners.contains(&fifo_id).then_some(source.clone()))
            .collect::<BTreeSet<_>>();
        if indexed_sources != *prior_sources {
            return Err("Sumeragi v2 outbound FIFO index changed before fanout update".to_owned());
        }
        let mut next = self.source_fifo_owners.clone();
        for source in prior_sources {
            let remove_source = {
                let owners = next
                    .get_mut(source)
                    .expect("preflighted exact-output source owner must remain present");
                let removed = owners.remove(&fifo_id);
                debug_assert!(removed);
                owners.is_empty()
            };
            if remove_source {
                next.remove(source);
            }
        }
        if updated_sources.iter().any(|source| {
            next.get(source)
                .is_some_and(|owners| owners.contains(&fifo_id))
        }) {
            return Err("Sumeragi v2 outbound FIFO registered one owner twice".to_owned());
        }
        for source in updated_sources {
            next.entry(source.clone()).or_default().insert(fifo_id);
        }
        Ok(next)
    }
    fn ownership_addition_load(
        &self,
        additions: &BTreeMap<ExactTargetReservation, usize>,
    ) -> Result<(usize, usize), String> {
        let mut added_units = 0usize;
        let mut added_shared_units = 0usize;
        for (reservation, added) in additions {
            if *added == 0 {
                return Err("Sumeragi v2 outbound ownership added an empty unit".to_owned());
            }
            added_units = added_units
                .checked_add(*added)
                .ok_or_else(|| "Sumeragi v2 outbound ownership units overflowed".to_owned())?;
            let current = self
                .reservation_owner_counts
                .get(reservation)
                .copied()
                .unwrap_or(0);
            let frozen_credit =
                usize::from(current == 0 && self.reserved_target_classes.contains(reservation));
            added_shared_units = added_shared_units
                .checked_add(added.checked_sub(frozen_credit).ok_or_else(|| {
                    "Sumeragi v2 outbound frozen credit exceeded its ownership".to_owned()
                })?)
                .ok_or_else(|| {
                    "Sumeragi v2 outbound shared ownership units overflowed".to_owned()
                })?;
        }
        Ok((added_units, added_shared_units))
    }
    fn ownership_capacity_available(
        &self,
        additions: &BTreeMap<ExactTargetReservation, usize>,
    ) -> Result<bool, String> {
        let (added_units, added_shared_units) = self.ownership_addition_load(additions)?;
        Ok(self
            .ownership_units
            .checked_add(added_units)
            .is_some_and(|units| units <= self.ownership_unit_capacity)
            && self
                .shared_ownership_units
                .checked_add(added_shared_units)
                .is_some_and(|units| units <= self.shared_ownership_unit_capacity))
    }
    fn ownership_state_after_additions(
        &self,
        additions: &BTreeMap<ExactTargetReservation, usize>,
    ) -> Result<(BTreeMap<ExactTargetReservation, usize>, usize, usize), String> {
        let (added_units, added_shared_units) = self.ownership_addition_load(additions)?;
        let next_ownership_units = self
            .ownership_units
            .checked_add(added_units)
            .filter(|units| *units <= self.ownership_unit_capacity)
            .ok_or_else(|| {
                "Sumeragi v2 outbound ownership exceeded its reserved geometry".to_owned()
            })?;
        let next_shared_ownership_units = self
            .shared_ownership_units
            .checked_add(added_shared_units)
            .filter(|units| *units <= self.shared_ownership_unit_capacity)
            .ok_or_else(|| {
                "Sumeragi v2 outbound ownership exceeded its reserved geometry".to_owned()
            })?;
        let mut next_reservation_owner_counts = self.reservation_owner_counts.clone();
        for (reservation, added) in additions {
            let count = next_reservation_owner_counts
                .entry(reservation.clone())
                .or_default();
            *count = count.checked_add(*added).ok_or_else(|| {
                "Sumeragi v2 outbound target/class multiplicity overflowed".to_owned()
            })?;
        }
        Ok((
            next_reservation_owner_counts,
            next_ownership_units,
            next_shared_ownership_units,
        ))
    }
    fn ownership_state_after_removals(
        &self,
        removals: &BTreeMap<ExactTargetReservation, usize>,
    ) -> Result<(BTreeMap<ExactTargetReservation, usize>, usize, usize), String> {
        let mut next_reservation_owner_counts = self.reservation_owner_counts.clone();
        let mut removed_units = 0usize;
        let mut removed_shared_units = 0usize;
        for (reservation, removed) in removals {
            if *removed == 0 {
                return Err("Sumeragi v2 outbound ownership removed an empty unit".to_owned());
            }
            let current = next_reservation_owner_counts
                .get(reservation)
                .copied()
                .ok_or_else(|| "Sumeragi v2 outbound ownership lost its target/class".to_owned())?;
            let remaining = current.checked_sub(*removed).ok_or_else(|| {
                "Sumeragi v2 outbound ownership removed too many target/class units".to_owned()
            })?;
            removed_units = removed_units
                .checked_add(*removed)
                .ok_or_else(|| "Sumeragi v2 outbound ownership removal overflowed".to_owned())?;
            let frozen_credit_removed =
                usize::from(remaining == 0 && self.reserved_target_classes.contains(reservation));
            removed_shared_units = removed_shared_units
                .checked_add(removed.checked_sub(frozen_credit_removed).ok_or_else(|| {
                    "Sumeragi v2 outbound frozen credit exceeded its removal".to_owned()
                })?)
                .ok_or_else(|| {
                    "Sumeragi v2 outbound shared ownership removal overflowed".to_owned()
                })?;
            if remaining == 0 {
                next_reservation_owner_counts.remove(reservation);
            } else {
                next_reservation_owner_counts.insert(reservation.clone(), remaining);
            }
        }
        let next_ownership_units = self
            .ownership_units
            .checked_sub(removed_units)
            .ok_or_else(|| "Sumeragi v2 outbound ownership total underflowed".to_owned())?;
        let next_shared_ownership_units = self
            .shared_ownership_units
            .checked_sub(removed_shared_units)
            .ok_or_else(|| "Sumeragi v2 outbound shared ownership underflowed".to_owned())?;
        Ok((
            next_reservation_owner_counts,
            next_ownership_units,
            next_shared_ownership_units,
        ))
    }
    fn ownership_state_after_replacement(
        &self,
        removals: &BTreeMap<ExactTargetReservation, usize>,
        additions: &BTreeMap<ExactTargetReservation, usize>,
    ) -> Result<Option<(BTreeMap<ExactTargetReservation, usize>, usize, usize)>, String> {
        let mut current_units = 0usize;
        let mut current_shared_units = 0usize;
        for (reservation, count) in &self.reservation_owner_counts {
            current_units = current_units.checked_add(*count).ok_or_else(|| {
                "Sumeragi v2 responder-control current ownership overflowed".to_owned()
            })?;
            let frozen_credit = usize::from(self.reserved_target_classes.contains(reservation));
            current_shared_units = current_shared_units
                .checked_add(count.checked_sub(frozen_credit).ok_or_else(|| {
                    "Sumeragi v2 responder-control current ownership lost its frozen credit"
                        .to_owned()
                })?)
                .ok_or_else(|| {
                    "Sumeragi v2 responder-control current shared ownership overflowed".to_owned()
                })?;
        }
        if current_units != self.ownership_units
            || current_shared_units != self.shared_ownership_units
        {
            return Err(
                "Sumeragi v2 responder-control replacement found inconsistent ownership".to_owned(),
            );
        }
        let mut next_reservation_owner_counts = self.reservation_owner_counts.clone();
        for (reservation, removed) in removals {
            if *removed == 0 {
                return Err("Sumeragi v2 outbound ownership replaced an empty unit".to_owned());
            }
            let current = next_reservation_owner_counts
                .get(reservation)
                .copied()
                .ok_or_else(|| {
                    "Sumeragi v2 responder-control replacement lost its target/class".to_owned()
                })?;
            let remaining = current.checked_sub(*removed).ok_or_else(|| {
                "Sumeragi v2 responder-control replacement removed too many units".to_owned()
            })?;
            if remaining == 0 {
                next_reservation_owner_counts.remove(reservation);
            } else {
                next_reservation_owner_counts.insert(reservation.clone(), remaining);
            }
        }
        for (reservation, added) in additions {
            if *added == 0 {
                return Err("Sumeragi v2 outbound ownership replaced with an empty unit".to_owned());
            }
            let count = next_reservation_owner_counts
                .entry(reservation.clone())
                .or_default();
            *count = count.checked_add(*added).ok_or_else(|| {
                "Sumeragi v2 responder-control replacement multiplicity overflowed".to_owned()
            })?;
        }
        let mut next_ownership_units = 0usize;
        let mut next_shared_ownership_units = 0usize;
        for (reservation, count) in &next_reservation_owner_counts {
            next_ownership_units = next_ownership_units.checked_add(*count).ok_or_else(|| {
                "Sumeragi v2 responder-control replacement ownership overflowed".to_owned()
            })?;
            let frozen_credit = usize::from(self.reserved_target_classes.contains(reservation));
            next_shared_ownership_units = next_shared_ownership_units
                .checked_add(count.checked_sub(frozen_credit).ok_or_else(|| {
                    "Sumeragi v2 responder-control replacement lost its frozen credit".to_owned()
                })?)
                .ok_or_else(|| {
                    "Sumeragi v2 responder-control replacement shared ownership overflowed"
                        .to_owned()
                })?;
        }
        if next_ownership_units > self.ownership_unit_capacity
            || next_shared_ownership_units > self.shared_ownership_unit_capacity
        {
            return Ok(None);
        }
        Ok(Some((
            next_reservation_owner_counts,
            next_ownership_units,
            next_shared_ownership_units,
        )))
    }
    fn remove_ownership_units(
        &mut self,
        removals: &BTreeMap<ExactTargetReservation, usize>,
    ) -> Result<(), String> {
        let (counts, units, shared_units) = self.ownership_state_after_removals(removals)?;
        self.reservation_owner_counts = counts;
        self.ownership_units = units;
        self.shared_ownership_units = shared_units;
        Ok(())
    }
    fn validate_fanout_bounds(&self, fanout: &PendingExactFanout) -> Result<(), String> {
        if fanout.fifo_id.is_some() {
            return Err("Sumeragi v2 outbound fanout already owns a FIFO identity".to_owned());
        }
        if fanout.messages.len() > self.max_messages_per_fanout
            || fanout.peers.len() > self.max_peers_per_fanout
        {
            return Err("Sumeragi v2 outbound fanout exceeds its protocol bound".to_owned());
        }
        if fanout.targets.iter().any(|target| target.parked) {
            return Err("Sumeragi v2 new outbound fanout contains a parked source".to_owned());
        }
        let reply_routes = fanout
            .targets
            .iter()
            .filter_map(|target| match &target.route {
                ExactTargetRoute::Reply(route) => Some(route),
                ExactTargetRoute::Topology => None,
            })
            .collect::<Vec<_>>();
        if !reply_routes.is_empty() {
            if reply_routes.len() != fanout.targets.len() {
                return Err(
                    "Sumeragi v2 outbound fanout mixed topology and reply routes".to_owned(),
                );
            }
            let mut authority = None;
            let mut sources = BTreeSet::new();
            for (route, peer) in reply_routes.iter().copied().zip(&fanout.peers) {
                if !route.is_active() {
                    return Err(
                        "Sumeragi v2 outbound reply fanout contains an inactive capability"
                            .to_owned(),
                    );
                }
                if route.semantic_target() != peer
                    || authority.is_some_and(|prior| !route.same_request_authority(prior))
                {
                    return Err(
                        "Sumeragi v2 outbound reply fanout changed actor or semantic target"
                            .to_owned(),
                    );
                }
                authority.get_or_insert(route);
                if !sources.insert(route.source_key()) {
                    return Err(
                        "Sumeragi v2 outbound reply fanout duplicated an authenticated source"
                            .to_owned(),
                    );
                }
            }
            let history = fanout.reply_routes.as_ref().ok_or_else(|| {
                "Sumeragi v2 outbound reply fanout lost its bounded route history".to_owned()
            })?;
            if history.semantic_target()
                != authority
                    .expect("reply routes established authority")
                    .semantic_target()
                || history.len() != reply_routes.len()
                || history.iter().any(|historical| {
                    !reply_routes
                        .iter()
                        .any(|target| target.same_delivery(historical))
                })
            {
                return Err(
                    "Sumeragi v2 outbound reply fanout route history changed live targets"
                        .to_owned(),
                );
            }
            if let Some(ownership) = &fanout.ingress_ownership
                && (!ownership.validate_exact() || !ownership.matches_reply_routes(Some(history)))
            {
                return Err(
                    "Sumeragi v2 outbound reply fanout changed fair-ingress ownership".to_owned(),
                );
            }
        } else if fanout.reply_routes.is_some() {
            return Err("Sumeragi v2 topology fanout retained reply-route history".to_owned());
        } else if fanout.ingress_ownership.is_some() {
            return Err("Sumeragi v2 topology fanout retained ingress ownership".to_owned());
        }
        if fanout.message_hashes.len() != fanout.messages.len()
            || fanout.message_classes.len() != fanout.messages.len()
            || fanout.message_class_suffixes.len().checked_sub(1) != Some(fanout.messages.len())
        {
            return Err("Sumeragi v2 outbound fanout lost its immutable message index".to_owned());
        }
        if fanout
            .messages
            .iter()
            .zip(&fanout.message_hashes)
            .zip(&fanout.message_classes)
            .any(|((message, expected_hash), expected_class)| {
                HashOf::new(message) != *expected_hash
                    || exact_output_class(message).as_ref() != Ok(expected_class)
            })
        {
            return Err("Sumeragi v2 outbound fanout changed its immutable messages".to_owned());
        }
        if fanout
            .message_class_suffixes
            .last()
            .is_none_or(|suffix| *suffix != 0)
            || fanout
                .message_classes
                .iter()
                .enumerate()
                .any(|(message_index, class)| {
                    let Some(expected_tail) = fanout.message_class_suffixes.get(message_index + 1)
                    else {
                        return true;
                    };
                    let expected_suffix = *expected_tail | exact_output_class_bit(*class);
                    fanout.message_class_suffixes.get(message_index) != Some(&expected_suffix)
                })
        {
            return Err(
                "Sumeragi v2 outbound fanout changed its reliable-class suffixes".to_owned(),
            );
        }
        if fanout.current_source_targets != fanout.expected_current_source_targets()? {
            return Err("Sumeragi v2 outbound fanout changed its local FIFO index".to_owned());
        }
        // Validate every future message class before consulting capacity. An
        // invalid route must never be disguised as temporary backpressure by
        // an already-full corridor.
        let _ = fanout.outstanding_sources()?;
        Ok(())
    }
    fn capacity_available_for(&self, fanout: &PendingExactFanout) -> Result<bool, String> {
        if self
            .fanouts
            .iter()
            .any(|pending| pending.can_coalesce_exact_topology_retry(fanout))
        {
            return Ok(true);
        }
        if let Some(pending) = self
            .fanouts
            .iter()
            .find(|pending| pending.can_coalesce_retry(fanout))
        {
            let plan = pending.reply_target_merge_plan(fanout)?;
            if !self.coalesced_target_geometry_available(pending, &plan)? {
                return Ok(false);
            }
            let additions =
                pending.coalesce_reservation_additions_for_plan(fanout, &plan.targets)?;
            return self.ownership_capacity_available(&additions);
        }
        self.ownership_capacity_available(&fanout.admission_reservation_counts()?)
    }
    fn coalesced_target_geometry_available(
        &self,
        pending: &PendingExactFanout,
        plan: &ReplyTargetMergePlan,
    ) -> Result<bool, String> {
        let appended = plan
            .targets
            .iter()
            .filter(|merge| matches!(merge, ReplyTargetMerge::Append { .. }))
            .count();
        let target_count = pending
            .targets
            .len()
            .checked_add(appended)
            .ok_or_else(|| "Sumeragi v2 reply target geometry overflowed".to_owned())?;
        Ok(target_count <= self.max_peers_per_fanout
            && target_count <= plan.reply_routes.source_capacity())
    }
    fn retains_retryable_sidecar_responder_control_for(
        &self,
        candidate: &PendingExactFanout,
    ) -> bool {
        candidate
            .retryable_certified_sidecar_responder_control_target()
            .is_some_and(|candidate_target| {
                self.fanouts.iter().any(|retained| {
                    retained.retryable_certified_sidecar_responder_control_target()
                        == Some(candidate_target)
                })
            })
    }
    fn stranded_responder_control_replacement_index(
        &self,
        candidate: &PendingExactFanout,
    ) -> Option<usize> {
        let candidate_target = candidate.retryable_certified_sidecar_responder_control_target()?;
        if !candidate.has_writable_reply_target() {
            return None;
        }
        self.fanouts.iter().position(|retained| {
            retained.retryable_certified_sidecar_responder_control_target()
                == Some(candidate_target)
                && retained.is_stranded_retryable_certified_sidecar_responder_control()
        })
    }
    fn responder_control_replacement_ownership(
        &self,
        retained_index: usize,
        candidate: &PendingExactFanout,
    ) -> Result<Option<(BTreeMap<ExactTargetReservation, usize>, usize, usize)>, String> {
        let retained = self
            .fanouts
            .get(retained_index)
            .ok_or_else(|| "Sumeragi v2 stranded responder control disappeared".to_owned())?;
        let retained_fifo_id = retained.fifo_id.ok_or_else(|| {
            "Sumeragi v2 stranded responder control lost its FIFO identity".to_owned()
        })?;
        let retained_sources = retained.outstanding_sources()?;
        let indexed_sources = self
            .source_fifo_owners
            .iter()
            .filter_map(|(source, owners)| {
                owners.contains(&retained_fifo_id).then_some(source.clone())
            })
            .collect::<BTreeSet<_>>();
        if indexed_sources != retained_sources {
            return Err(
                "Sumeragi v2 stranded responder control changed its FIFO ownership".to_owned(),
            );
        }
        self.ownership_state_after_replacement(
            &retained.outstanding_reservation_counts()?,
            &candidate.outstanding_reservation_counts()?,
        )
    }
    fn responder_control_replacement_available(
        &self,
        candidate: &PendingExactFanout,
    ) -> Result<bool, String> {
        let Some(retained_index) = self.stranded_responder_control_replacement_index(candidate)
        else {
            return Ok(false);
        };
        Ok(self
            .responder_control_replacement_ownership(retained_index, candidate)?
            .is_some())
    }
    fn responder_control_replacement_plan(
        &self,
        retained_index: usize,
        candidate: &PendingExactFanout,
    ) -> Result<Option<ResponderControlReplacementPlan>, String> {
        let Some((reservation_owner_counts, ownership_units, shared_ownership_units)) =
            self.responder_control_replacement_ownership(retained_index, candidate)?
        else {
            return Ok(None);
        };
        let retained = self
            .fanouts
            .get(retained_index)
            .expect("located stranded responder control must remain present");
        let retained_fifo_id = retained
            .fifo_id
            .expect("preflighted responder control retains its FIFO identity");
        let replacement_fifo_id = self.next_fanout_fifo_id;
        let next_fanout_fifo_id = replacement_fifo_id.checked_add(1).ok_or_else(|| {
            "Sumeragi v2 outbound FIFO must rebase before responder-control replacement".to_owned()
        })?;
        if self
            .source_fifo_owners
            .values()
            .any(|owners| owners.contains(&replacement_fifo_id))
        {
            return Err(
                "Sumeragi v2 responder-control replacement reused a live FIFO identity".to_owned(),
            );
        }
        let fanout_count = self.fanouts.len();
        if fanout_count == 0 || self.next_fanout_index >= fanout_count {
            return Err(
                "Sumeragi v2 responder-control replacement found an invalid scheduler cursor"
                    .to_owned(),
            );
        }
        let next_fanout_index = if fanout_count == 1 {
            0
        } else if self.next_fanout_index == retained_index {
            if retained_index + 1 < fanout_count {
                // Removing the retained slot shifts its successor into the
                // same index. The fresh replacement rejoins at the tail.
                retained_index
            } else {
                // The retired slot was last, so continue at the old wrap
                // point instead of granting the replacement that position.
                0
            }
        } else if self.next_fanout_index > retained_index {
            self.next_fanout_index - 1
        } else {
            self.next_fanout_index
        };
        let retained_sources = retained.outstanding_sources()?;
        let replacement_sources = candidate.outstanding_sources()?;
        let mut source_fifo_owners = self.source_fifo_owners.clone();
        for source in &retained_sources {
            let remove_source = {
                let owners = source_fifo_owners.get_mut(source).ok_or_else(|| {
                    "Sumeragi v2 responder-control replacement lost a registered source".to_owned()
                })?;
                if !owners.remove(&retained_fifo_id) {
                    return Err(
                        "Sumeragi v2 responder-control replacement lost its registered owner"
                            .to_owned(),
                    );
                }
                owners.is_empty()
            };
            if remove_source {
                source_fifo_owners.remove(source);
            }
        }
        for source in replacement_sources {
            if !source_fifo_owners
                .entry(source)
                .or_default()
                .insert(replacement_fifo_id)
            {
                return Err(
                    "Sumeragi v2 responder-control replacement registered one source twice"
                        .to_owned(),
                );
            }
        }
        Ok(Some(ResponderControlReplacementPlan {
            retained_index,
            replacement_fifo_id,
            next_fanout_fifo_id,
            next_fanout_index,
            source_fifo_owners,
            reservation_owner_counts,
            ownership_units,
            shared_ownership_units,
        }))
    }
    fn commit_stranded_responder_control_replacement(
        &mut self,
        mut candidate: PendingExactFanout,
    ) -> Result<Option<PendingExactFanout>, String> {
        let Some(retained_index) = self.stranded_responder_control_replacement_index(&candidate)
        else {
            return Ok(None);
        };
        // Capacity failure must not rebase live FIFO identities. Establish
        // that the replacement fits at the same liveness snapshot before the
        // only preparatory mutation. Reply writability is monotonic within a
        // tenure, so the plan below deliberately reuses this retained index
        // instead of rereading external route state after a FIFO rebase.
        if self
            .responder_control_replacement_ownership(retained_index, &candidate)?
            .is_none()
        {
            return Ok(None);
        }
        if self.fanouts.is_empty() || self.next_fanout_index >= self.fanouts.len() {
            return Err(
                "Sumeragi v2 responder-control replacement found an invalid scheduler cursor"
                    .to_owned(),
            );
        }
        if self.next_fanout_fifo_id == ExactFanoutFifoId::MAX {
            self.rebase_source_fifo()?;
        }
        let Some(plan) = self.responder_control_replacement_plan(retained_index, &candidate)?
        else {
            return Ok(None);
        };
        candidate.fifo_id = Some(plan.replacement_fifo_id);
        let retired = self
            .fanouts
            .remove(plan.retained_index)
            .expect("planned stranded responder control must remain present");
        // This is new authenticated-source work. Appending it keeps deque
        // round-robin age aligned with the fresh source FIFO identity, even
        // after a later FIFO rebase.
        self.fanouts.push_back(candidate);
        self.next_fanout_fifo_id = plan.next_fanout_fifo_id;
        self.next_fanout_index = plan.next_fanout_index;
        self.source_fifo_owners = plan.source_fifo_owners;
        self.reservation_owner_counts = plan.reservation_owner_counts;
        self.ownership_units = plan.ownership_units;
        self.shared_ownership_units = plan.shared_ownership_units;
        Ok(Some(retired))
    }
    fn replace_stranded_responder_control(
        &mut self,
        candidate: PendingExactFanout,
    ) -> Result<bool, String> {
        let Some(retired) = self.commit_stranded_responder_control_replacement(candidate)? else {
            return Ok(false);
        };
        // Actor-ticket destruction can emit cancellation. Keep that external
        // side effect strictly after every worker-owned index is committed.
        drop(retired);
        Ok(true)
    }
    fn can_enqueue(&self, fanout: &PendingExactFanout) -> Result<bool, String> {
        self.validate_fanout_bounds(fanout)?;
        if self
            .stranded_responder_control_replacement_index(fanout)
            .is_some()
        {
            return self.responder_control_replacement_available(fanout);
        }
        if self
            .fanouts
            .iter()
            .any(|pending| pending.can_coalesce_exact_topology_retry(fanout))
        {
            return Ok(true);
        }
        if self
            .fanouts
            .iter()
            .any(|pending| pending.can_coalesce_retry(fanout))
        {
            return self.capacity_available_for(fanout);
        }
        if self.retains_retryable_sidecar_responder_control_for(fanout) {
            // Preserve one bounded successor in lane work while the incumbent
            // still has a writer or a pending flush. Consuming a distinct
            // control here would lose the newest cumulative CloseAck or the
            // GenerationHint for the request hash the client actually retains.
            return Ok(false);
        }
        self.capacity_available_for(fanout)
    }
    fn validate_owned_reply_transfer(
        &self,
        fanout: &mut PendingExactFanout,
    ) -> Result<bool, String> {
        loop {
            if fanout.retain_active_unowned_reply_targets()? == 0 {
                return Ok(false);
            }
            match self.validate_fanout_bounds(fanout) {
                Ok(()) => return Ok(true),
                Err(error)
                    if fanout.targets.iter().any(
                        |target| matches!(&target.route, ExactTargetRoute::Reply(route) if !route.is_active()),
                    ) =>
                {
                    // A tenure retired between pruning and validation. Active
                    // is monotonic, so each retry removes at least one route.
                    drop(error);
                }
                Err(error) => return Err(error),
            }
        }
    }
    fn can_enqueue_owned_reply_transfer(
        &self,
        mut fanout: PendingExactFanout,
    ) -> Result<bool, String> {
        if !self.validate_owned_reply_transfer(&mut fanout)? {
            return Ok(true);
        }
        self.project_sidecar_receipt_completions(&mut fanout)?;
        if self
            .stranded_responder_control_replacement_index(&fanout)
            .is_some()
        {
            return self.responder_control_replacement_available(&fanout);
        }
        if self
            .fanouts
            .iter()
            .any(|pending| pending.can_coalesce_retry(&fanout))
        {
            return self.capacity_available_for(&fanout);
        }
        if self.retains_retryable_sidecar_responder_control_for(&fanout) {
            return Ok(false);
        }
        self.capacity_available_for(&fanout)
    }
    fn enqueue(&mut self, fanout: PendingExactFanout) -> Result<ExactFanoutOwnership, String> {
        self.validate_fanout_bounds(&fanout)?;
        self.enqueue_validated(fanout)
    }
    fn enqueue_owned_reply_transfer(
        &mut self,
        mut fanout: PendingExactFanout,
    ) -> Result<ExactFanoutOwnership, String> {
        if !self.validate_owned_reply_transfer(&mut fanout)? {
            return Ok(ExactFanoutOwnership::Owned);
        }
        self.project_sidecar_receipt_completions(&mut fanout)?;
        self.enqueue_validated(fanout)
    }
    /// Coalesce post-flush reply redelivery while ordinary fanout ownership and
    /// cursor stay on the target; only the receipt needs terminal projection.
    fn project_sidecar_receipt_completions(
        &self,
        fanout: &mut PendingExactFanout,
    ) -> Result<(), String> {
        let [message] = fanout.messages.as_slice() else {
            return Ok(());
        };
        let NetworkMessage::CertifiedMergeSidecar(message) = message else {
            return Ok(());
        };
        let CertifiedMergeSidecarMessage::Chunk(_) = message.as_ref() else {
            return Ok(());
        };
        let completed_cursor = fanout.messages.len();
        let completed_message_cursor = u64::try_from(completed_cursor)
            .map_err(|_| "Sumeragi v2 sidecar replay cursor exceeded u64".to_owned())?;
        let mut completed_routes = Vec::new();
        let mut projected_completion = false;
        for target in &mut fanout.targets {
            if target.message_index == completed_cursor {
                continue;
            }
            if target.message_index != 0 || target.current.is_some() || target.ticket.is_some() {
                return Err(
                    "Sumeragi v2 sidecar replay carried pre-existing exact-output state".to_owned(),
                );
            }
            let ExactTargetRoute::Reply(route) = &target.route else {
                continue;
            };
            let source_terminal = self.admitted_sidecar_chunks.iter().any(|admission| {
                admission.matches_materialized_chunk(message) && admission.is_bound_to_source(route)
            });
            if source_terminal {
                target.message_index = completed_cursor;
                completed_routes.push(route.clone());
                projected_completion = true;
            }
        }
        if !completed_routes.is_empty() {
            if let Some(ownership) = fanout.ingress_ownership.as_mut() {
                for route in &completed_routes {
                    if !ownership.advance_reply_cursors(route, completed_message_cursor, 0) {
                        return Err(
                            "Sumeragi v2 retained sidecar flush lost fair-ingress ownership"
                                .to_owned(),
                        );
                    }
                }
            }
        }
        if projected_completion {
            fanout.rebuild_current_source_targets()?;
        }
        Ok(())
    }
    fn enqueue_validated(
        &mut self,
        mut fanout: PendingExactFanout,
    ) -> Result<ExactFanoutOwnership, String> {
        if fanout.is_complete() {
            return Ok(ExactFanoutOwnership::Owned);
        }
        if self
            .stranded_responder_control_replacement_index(&fanout)
            .is_some()
        {
            return self
                .replace_stranded_responder_control(fanout)
                .map(|replaced| {
                    if replaced {
                        ExactFanoutOwnership::Owned
                    } else {
                        ExactFanoutOwnership::SourceRetained
                    }
                });
        }
        if self
            .fanouts
            .iter()
            .any(|pending| pending.can_coalesce_exact_topology_retry(&fanout))
        {
            return Ok(ExactFanoutOwnership::Owned);
        }
        if let Some(index) = self
            .fanouts
            .iter()
            .position(|pending| pending.can_coalesce_retry(&fanout))
        {
            let (fifo_id, prior_sources, plan, preview, ownership_additions) = {
                let pending = self
                    .fanouts
                    .get(index)
                    .expect("located exact-output retry must remain present");
                if pending.current_source_targets != pending.expected_current_source_targets()? {
                    return Err(
                        "Sumeragi v2 retained fanout changed its local FIFO index".to_owned()
                    );
                }
                let fifo_id = pending.fifo_id.ok_or_else(|| {
                    "Sumeragi v2 retained fanout lost its FIFO identity".to_owned()
                })?;
                let plan = pending.reply_target_merge_plan(&fanout)?;
                let preview = pending.preview_coalesce_plan(&fanout, &plan)?;
                let ownership_additions =
                    pending.coalesce_reservation_additions_for_plan(&fanout, &plan.targets)?;
                (
                    fifo_id,
                    pending.outstanding_sources()?,
                    plan,
                    preview,
                    ownership_additions,
                )
            };
            let next_source_fifo_owners = self.source_fifo_owners_after_fanout_replacement(
                fifo_id,
                &prior_sources,
                &preview.outstanding_sources,
            )?;
            if plan.targets.is_empty() {
                self.fanouts
                    .get_mut(index)
                    .expect("located exact-output retry must remain present")
                    .commit_coalesce_plan(&fanout, &plan, preview.current_source_targets);
                self.source_fifo_owners = next_source_fifo_owners;
                return Ok(ExactFanoutOwnership::Owned);
            }
            if !self.coalesced_target_geometry_available(
                self.fanouts
                    .get(index)
                    .expect("located exact-output retry must remain present"),
                &plan,
            )? || !self.ownership_capacity_available(&ownership_additions)?
            {
                return Ok(ExactFanoutOwnership::SourceRetained);
            }
            let (next_reservation_owner_counts, next_ownership_units, next_shared_ownership_units) =
                self.ownership_state_after_additions(&ownership_additions)?;
            self.fanouts
                .get_mut(index)
                .expect("located exact-output retry must remain present")
                .commit_coalesce_plan(&fanout, &plan, preview.current_source_targets);
            self.source_fifo_owners = next_source_fifo_owners;
            self.reservation_owner_counts = next_reservation_owner_counts;
            self.ownership_units = next_ownership_units;
            self.shared_ownership_units = next_shared_ownership_units;
            return Ok(ExactFanoutOwnership::Owned);
        }
        if self.retains_retryable_sidecar_responder_control_for(&fanout) {
            // At most one responder control per semantic target owns this
            // corridor. Lane work retains the distinct successor until the
            // incumbent drains or becomes safely replaceable.
            return Ok(ExactFanoutOwnership::SourceRetained);
        }
        let ownership_additions = fanout.outstanding_reservation_counts()?;
        if !self.ownership_capacity_available(&ownership_additions)? {
            return Ok(ExactFanoutOwnership::SourceRetained);
        }
        let (next_reservation_owner_counts, next_ownership_units, next_shared_ownership_units) =
            self.ownership_state_after_additions(&ownership_additions)?;
        let sources = fanout.outstanding_sources()?;
        let fifo_id = self.allocate_fanout_fifo_id()?;
        let mut next_source_fifo_owners = self.source_fifo_owners.clone();
        debug_assert!(
            next_source_fifo_owners
                .values()
                .all(|owners| !owners.contains(&fifo_id))
        );
        for source in sources {
            next_source_fifo_owners
                .entry(source)
                .or_default()
                .insert(fifo_id);
        }
        fanout.fifo_id = Some(fifo_id);
        self.source_fifo_owners = next_source_fifo_owners;
        self.reservation_owner_counts = next_reservation_owner_counts;
        self.ownership_units = next_ownership_units;
        self.shared_ownership_units = next_shared_ownership_units;
        self.fanouts.push_back(fanout);
        Ok(ExactFanoutOwnership::Owned)
    }
    fn handoff_applied_height_to_durable_reconstruction(
        &mut self,
        artifact: &wire::finality::V2FinalityArtifact,
        durable_lane_authority: Option<&DurableLaneRolloverAuthority>,
        durable_history: Option<&Kura>,
    ) -> Result<usize, String> {
        let mut remaining_posts = 0usize;
        let mut expected_source_fifo_owners =
            BTreeMap::<ExactTargetSource, BTreeSet<ExactFanoutFifoId>>::new();
        let mut expected_reservation_owner_counts =
            BTreeMap::<ExactTargetReservation, usize>::new();
        for fanout in &self.fanouts {
            if let Some(ownership) = &fanout.ingress_ownership
                && (fanout
                    .reply_routes
                    .as_ref()
                    .is_none_or(|routes| !ownership.matches_reply_routes(Some(routes)))
                    || !ownership.validate_exact())
            {
                return Err(
                    "Sumeragi v2 finalized output changed fair-ingress ownership".to_owned(),
                );
            }
            if fanout.message_hashes.len() != fanout.messages.len()
                || fanout
                    .messages
                    .iter()
                    .zip(&fanout.message_hashes)
                    .any(|(message, expected_hash)| HashOf::new(message) != *expected_hash)
            {
                return Err(
                    "Sumeragi v2 retained output changed before finality handoff".to_owned(),
                );
            }
            let fifo_id = fanout.fifo_id.ok_or_else(|| {
                "Sumeragi v2 retained fanout lost its FIFO identity before finality handoff"
                    .to_owned()
            })?;
            for source in fanout.outstanding_sources()? {
                expected_source_fifo_owners
                    .entry(source)
                    .or_default()
                    .insert(fifo_id);
            }
            for (reservation, count) in fanout.outstanding_reservation_counts()? {
                let aggregate = expected_reservation_owner_counts
                    .entry(reservation)
                    .or_default();
                *aggregate = aggregate.checked_add(count).ok_or_else(|| {
                    "Sumeragi v2 outbound handoff ownership count overflowed".to_owned()
                })?;
            }
            applied_height_reconstruction_covers(
                &fanout.messages,
                &fanout.semantic_peers(),
                &fanout.rollover_claim,
                artifact,
                durable_lane_authority,
                durable_history,
            )?;
            for (target_index, target) in fanout.targets.iter().enumerate() {
                if target.message_index > fanout.messages.len() {
                    return Err(
                        "Sumeragi v2 exact-output target advanced beyond its fanout".to_owned()
                    );
                }
                if target.ticket.is_some() && target.current.is_none() {
                    return Err("Sumeragi v2 exact-output ticket lost its returned post".to_owned());
                }
                if target.parked
                    && (!matches!(
                        &target.route,
                        ExactTargetRoute::Reply(route)
                            if !route.is_active() || !route.is_reply_writable()
                    ) || target.current.is_some()
                        || target.ticket.is_some()
                        || fanout.target_is_complete(target_index))
                {
                    return Err(
                        "Sumeragi v2 parked reply source changed before finality handoff"
                            .to_owned(),
                    );
                }
                if let Some(current) = &target.current {
                    if fanout.peers.get(target_index) != Some(&current.peer_id) {
                        return Err(
                            "Sumeragi v2 exact-output target changed before finality handoff"
                                .to_owned(),
                        );
                    }
                    let expected_hash = fanout
                        .message_hashes
                        .get(target.message_index)
                        .ok_or_else(|| {
                            "Sumeragi v2 exact-output target has no expected payload identity"
                                .to_owned()
                        })?;
                    if HashOf::new(&current.data) != *expected_hash {
                        return Err(
                            "Sumeragi v2 returned output changed before finality handoff"
                                .to_owned(),
                        );
                    }
                }
                if let Some(pending_flush) = &target.pending_flush {
                    if target.current.is_some() || target.ticket.is_some() {
                        return Err(
                            "Sumeragi v2 writer flush shared tenure-bound actor ownership"
                                .to_owned(),
                        );
                    }
                    let data = fanout.messages.get(target.message_index).ok_or_else(|| {
                        "Sumeragi v2 writer flush advanced beyond its immutable payload".to_owned()
                    })?;
                    let peer_id = fanout.peers.get(target_index).ok_or_else(|| {
                        "Sumeragi v2 writer flush lost its semantic target".to_owned()
                    })?;
                    let canonical_post = Post {
                        data: data.clone(),
                        peer_id: peer_id.clone(),
                        priority: Priority::High,
                    };
                    let ExactTargetRoute::Reply(route) = &target.route else {
                        return Err(
                            "Sumeragi v2 topology target retained a reply writer flush".to_owned()
                        );
                    };
                    if !pending_flush
                        .flush_ack
                        .identity()
                        .is_bound_to_canonical_reply(&canonical_post)
                        || pending_flush.flush_ack.identity().source_key() != route.source_key()
                        || pending_flush.reply_writer_timeout_attempt
                            != target.reply_writer_timeout_attempt
                        || pending_flush
                            .flush_ack
                            .identity()
                            .reply_writer_timeout_attempt()
                            != pending_flush.reply_writer_timeout_attempt
                        || pending_flush
                            .sidecar_admission
                            .as_ref()
                            .is_some_and(|admission| {
                                !admission.matches_ack_identity(pending_flush.flush_ack.identity())
                            })
                    {
                        return Err(
                            "Sumeragi v2 writer flush changed before finality handoff".to_owned()
                        );
                    }
                }
                for _message in &fanout.messages[target.message_index..] {
                    remaining_posts = remaining_posts.checked_add(1).ok_or_else(|| {
                        "Sumeragi v2 applied-height output count overflowed".to_owned()
                    })?;
                }
            }
        }
        if self.source_fifo_owners != expected_source_fifo_owners {
            return Err(
                "Sumeragi v2 outbound FIFO index changed before finality handoff".to_owned(),
            );
        }
        if self.reservation_owner_counts != expected_reservation_owner_counts {
            return Err(
                "Sumeragi v2 outbound ownership index changed before finality handoff".to_owned(),
            );
        }
        let mut expected_ownership_units = 0usize;
        let mut expected_shared_ownership_units = 0usize;
        for (reservation, count) in &expected_reservation_owner_counts {
            expected_ownership_units = expected_ownership_units
                .checked_add(*count)
                .ok_or_else(|| "Sumeragi v2 outbound handoff units overflowed".to_owned())?;
            let frozen_credit = usize::from(self.reserved_target_classes.contains(reservation));
            expected_shared_ownership_units = expected_shared_ownership_units
                .checked_add(count.checked_sub(frozen_credit).ok_or_else(|| {
                    "Sumeragi v2 outbound handoff lost its frozen ownership credit".to_owned()
                })?)
                .ok_or_else(|| "Sumeragi v2 outbound handoff shared units overflowed".to_owned())?;
        }
        if self.ownership_units != expected_ownership_units
            || self.shared_ownership_units != expected_shared_ownership_units
        {
            return Err(
                "Sumeragi v2 outbound ownership totals changed before finality handoff".to_owned(),
            );
        }
        // Pending sidecar writer occurrences remain in their target's suffix
        // and were counted above. Only flushed receipts live beyond a fanout.
        let sidecar_completions = self.admitted_sidecar_chunks.len();
        remaining_posts = remaining_posts
            .checked_add(sidecar_completions)
            .ok_or_else(|| "Sumeragi v2 applied-height output count overflowed".to_owned())?;
        self.fanouts.clear();
        // The per-height lane transport and worker are dropped together.
        // Pending target acknowledgements and flushed-but-unapplied receipts
        // are atomically superseded by the typed Kura reconstruction claim;
        // retaining either here would let an unresponsive requester block the
        // decided height's successor activation.
        self.admitted_sidecar_chunks.clear();
        self.next_fanout_index = 0;
        self.next_fanout_fifo_id = 0;
        self.source_fifo_owners.clear();
        self.reservation_owner_counts.clear();
        self.ownership_units = 0;
        self.shared_ownership_units = 0;
        Ok(remaining_posts)
    }
    fn target_is_global_head(
        &self,
        fanout_index: usize,
        target_index: usize,
    ) -> Result<bool, String> {
        let fanout = self
            .fanouts
            .get(fanout_index)
            .ok_or_else(|| "Sumeragi v2 exact-output fanout disappeared".to_owned())?;
        if !fanout.target_is_local_head(target_index)? {
            return Ok(false);
        }
        let source = fanout.current_target_source(target_index)?;
        let fifo_id = fanout
            .fifo_id
            .ok_or_else(|| "Sumeragi v2 retained fanout lost its FIFO identity".to_owned())?;
        let owners = self
            .source_fifo_owners
            .get(&source)
            .ok_or_else(|| "Sumeragi v2 outbound FIFO lost its current source".to_owned())?;
        if !owners.contains(&fifo_id) {
            return Err("Sumeragi v2 outbound FIFO lost its current owner".to_owned());
        }
        let oldest_owner = owners
            .first()
            .expect("non-empty exact-output source owner set has a first entry");
        Ok(*oldest_owner == fifo_id)
    }
    fn next_schedulable_target(
        &self,
        blocked_sources: &BTreeSet<ExactTargetSource>,
    ) -> Result<Option<(usize, usize)>, String> {
        let fanout_count = self.fanouts.len();
        for fanout_offset in 0..fanout_count {
            let fanout_index = (self.next_fanout_index + fanout_offset) % fanout_count;
            let fanout = self
                .fanouts
                .get(fanout_index)
                .expect("round-robin exact fanout index must be present");
            for target_offset in 0..fanout.targets.len() {
                let target_index =
                    (fanout.next_target_index + target_offset) % fanout.targets.len();
                if fanout.target_is_complete(target_index) {
                    continue;
                }
                if fanout.targets[target_index].parked
                    || fanout.targets[target_index].pending_flush.is_some()
                {
                    continue;
                }
                let source = fanout.current_target_source(target_index)?;
                if !blocked_sources.contains(&source)
                    && self.target_is_global_head(fanout_index, target_index)?
                {
                    return Ok(Some((fanout_index, target_index)));
                }
            }
        }
        Ok(None)
    }
    /// Whether a FIFO head awaits reply-route activity; later local fanouts may
    /// proceed while it waits for reconnect or flush acknowledgement.
    fn has_quiescent_fifo_head(&self) -> Result<bool, String> {
        for (fanout_index, fanout) in self.fanouts.iter().enumerate() {
            for (target_index, target) in fanout.targets.iter().enumerate() {
                if fanout.target_is_complete(target_index)
                    || (!target.parked && target.pending_flush.is_none())
                {
                    continue;
                }
                if self.target_is_global_head(fanout_index, target_index)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    fn next_inactive_reply_target(&self) -> Option<(usize, usize)> {
        let fanout_count = self.fanouts.len();
        for fanout_offset in 0..fanout_count {
            let fanout_index = (self.next_fanout_index + fanout_offset) % fanout_count;
            let fanout = self
                .fanouts
                .get(fanout_index)
                .expect("round-robin exact fanout index must be present");
            for target_offset in 0..fanout.targets.len() {
                let target_index =
                    (fanout.next_target_index + target_offset) % fanout.targets.len();
                if fanout.target_is_complete(target_index) || fanout.targets[target_index].parked {
                    continue;
                }
                if matches!(
                    &fanout.targets[target_index].route,
                    ExactTargetRoute::Reply(route) if !route.is_active()
                ) {
                    return Some((fanout_index, target_index));
                }
            }
        }
        None
    }
    fn advance_after_attempt(
        &mut self,
        fanout_index: usize,
        target_index: usize,
        admitted_source: Option<&ExactTargetSource>,
    ) -> Result<(), String> {
        let (fanout_complete, released_reservation, released_source_owner) = {
            let fanout = self
                .fanouts
                .get_mut(fanout_index)
                .expect("attempted exact fanout must remain present");
            fanout.advance_target_cursor(target_index);
            let fanout_complete = fanout.is_complete();
            let released_reservation = if let Some(source) = admitted_source {
                let target = fanout
                    .targets
                    .get(target_index)
                    .ok_or_else(|| "Sumeragi v2 exact-output target disappeared".to_owned())?;
                let remaining_mask = *fanout
                    .message_class_suffixes
                    .get(target.message_index)
                    .ok_or_else(|| {
                        "Sumeragi v2 exact-output target advanced beyond its class suffix"
                            .to_owned()
                    })?;
                if remaining_mask & exact_output_class_bit(source.class) != 0 {
                    None
                } else {
                    let semantic_target = fanout
                        .peers
                        .get(target_index)
                        .expect("selected exact-output target must retain its peer");
                    let reservation = fanout.target_reservation(semantic_target, source.class);
                    if reservation.kind == ExactTargetReservationKind::SidecarReplyControl
                        && fanout
                            .outstanding_reservation_counts()?
                            .contains_key(&reservation)
                    {
                        None
                    } else {
                        Some(reservation)
                    }
                }
            } else {
                None
            };
            let released_source_owner = if let Some(source) = admitted_source {
                if fanout.owns_source(source)? {
                    None
                } else {
                    Some(fanout.fifo_id.ok_or_else(|| {
                        "Sumeragi v2 retained fanout lost its FIFO identity".to_owned()
                    })?)
                }
            } else {
                None
            };
            Ok::<_, String>((fanout_complete, released_reservation, released_source_owner))
        }?;
        if let Some(reservation) = released_reservation {
            self.remove_ownership_units(&BTreeMap::from([(reservation, 1)]))?;
        }
        if let (Some(fifo_id), Some(source)) = (released_source_owner, admitted_source) {
            self.unregister_source_fifo_owner(fifo_id, source)?;
        }
        if fanout_complete {
            let fifo_id = self
                .fanouts
                .get(fanout_index)
                .and_then(|fanout| fanout.fifo_id)
                .ok_or_else(|| "Sumeragi v2 completed fanout lost its FIFO identity".to_owned())?;
            if self
                .source_fifo_owners
                .values()
                .any(|owners| owners.contains(&fifo_id))
            {
                return Err("Sumeragi v2 completed fanout retained a FIFO source".to_owned());
            }
            self.fanouts
                .remove(fanout_index)
                .expect("completed exact fanout must remain present");
            self.next_fanout_index = if self.fanouts.is_empty() {
                0
            } else {
                fanout_index % self.fanouts.len()
            };
        } else {
            self.next_fanout_index = (fanout_index + 1) % self.fanouts.len();
        }
        Ok(())
    }
    fn park_unwritable_reply_target(
        &mut self,
        fanout_index: usize,
        target_index: usize,
    ) -> Result<(), String> {
        {
            let fanout = self
                .fanouts
                .get(fanout_index)
                .ok_or_else(|| "Sumeragi v2 draining fanout disappeared".to_owned())?;
            let target = fanout
                .targets
                .get(target_index)
                .ok_or_else(|| "Sumeragi v2 draining reply target disappeared".to_owned())?;
            match &target.route {
                // Reply writability is monotone within one tenure. The final
                // receiver may retire after the actor reports Unavailable or
                // closes a flush acknowledgement, so an inactive route is a
                // valid later observation of the same draining occurrence.
                ExactTargetRoute::Reply(route) if !route.is_reply_writable() => {}
                ExactTargetRoute::Reply(_) => {
                    return Err("Sumeragi v2 attempted to park a writable reply route".to_owned());
                }
                ExactTargetRoute::Topology => {
                    return Err("Sumeragi v2 attempted to park a topology target".to_owned());
                }
            }
            if target.parked || target.pending_flush.is_some() {
                return Err(
                    "Sumeragi v2 attempted to park an owned or already parked reply target"
                        .to_owned(),
                );
            }
            if fanout.target_is_complete(target_index) || fanout.fifo_id.is_none() {
                return Err("Sumeragi v2 draining reply target lost cursor ownership".to_owned());
            }
            let _ = fanout.outstanding_sources()?;
            let _ = fanout.outstanding_reservation_counts()?;
        }
        let fanout = self
            .fanouts
            .get_mut(fanout_index)
            .expect("preflighted draining fanout must remain present");
        let target = fanout
            .targets
            .get_mut(target_index)
            .expect("preflighted draining target must remain present");
        target.current = None;
        target.ticket = None;
        target.parked = true;
        // Preserve route history, immutable payload, message cursor, FIFO age,
        // and reservation ownership. A newer same-source tenure updates this
        // exact target and retries its current item.
        fanout.advance_target_cursor(target_index);
        self.next_fanout_index = (fanout_index + 1) % self.fanouts.len();
        Ok(())
    }
    fn retire_inactive_reply_target(
        &mut self,
        fanout_index: usize,
        target_index: usize,
    ) -> Result<(), String> {
        {
            let fanout = self
                .fanouts
                .get(fanout_index)
                .ok_or_else(|| "Sumeragi v2 retired fanout disappeared".to_owned())?;
            let target = fanout
                .targets
                .get(target_index)
                .ok_or_else(|| "Sumeragi v2 retired reply target disappeared".to_owned())?;
            match &target.route {
                ExactTargetRoute::Reply(route) if !route.is_active() => {}
                ExactTargetRoute::Reply(_) => {
                    return Err("Sumeragi v2 attempted to retire an active reply route".to_owned());
                }
                ExactTargetRoute::Topology => {
                    return Err("Sumeragi v2 attempted to retire a topology target".to_owned());
                }
            }
            if target.parked {
                return Err("Sumeragi v2 attempted to park one reply target twice".to_owned());
            }
            if fanout.reply_routes.is_none() {
                return Err(
                    "Sumeragi v2 retired reply fanout lost its bounded route history".to_owned(),
                );
            }
            if fanout.current_source_targets != fanout.expected_current_source_targets()? {
                return Err(
                    "Sumeragi v2 retired reply fanout changed its local FIFO index".to_owned(),
                );
            }
            if fanout.target_is_complete(target_index) {
                return Err("Sumeragi v2 attempted to park a completed reply source".to_owned());
            }
            if fanout.fifo_id.is_none() {
                return Err("Sumeragi v2 retired fanout lost its FIFO identity".to_owned());
            }
            // Validate the retained source and reservation projections before
            // changing tenure-bound state. Parking preserves both projections.
            let _ = fanout.outstanding_sources()?;
            let _ = fanout.outstanding_reservation_counts()?;
        }
        let fanout = self
            .fanouts
            .get_mut(fanout_index)
            .expect("retired exact fanout must remain present");
        let (_, prune_receipt) = fanout
            .reply_routes
            .as_mut()
            .expect("preflighted reply fanout must retain its route history")
            .retain_active_with_receipt();
        if let Some(ownership) = fanout.ingress_ownership.as_mut() {
            let Some(projected_routes) = ownership.project_retained_reply_routes(prune_receipt)
            else {
                return Err(
                    "Sumeragi v2 retired reply target lost fair-ingress ownership".to_owned(),
                );
            };
            fanout.reply_routes = Some(projected_routes);
        }
        let target = fanout
            .targets
            .get_mut(target_index)
            .expect("retired exact target must remain present");
        target.current = None;
        target.ticket = None;
        target.parked = true;
        // Only the scheduling cursor advances. The message cursor, local/global
        // source FIFO ownership, and reservation ownership stay unchanged so a
        // reconnect retries this exact current item.
        fanout.advance_target_cursor(target_index);
        self.next_fanout_index = (fanout_index + 1) % self.fanouts.len();
        Ok(())
    }
    /// Drive exact output fairly until drained, blocked, or the deterministic budget is spent.
    fn drive_with_budget_ack<Attempt>(
        &mut self,
        attempt_budget: usize,
        mut attempt: Attempt,
    ) -> Result<ExactOutputDriveOutcome, String>
    where
        Attempt: FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
            &ExactTargetRoute,
            u8,
        ) -> Result<
            ExactOutputAttemptOutcome,
            NetworkActorAdmissionError<Post<NetworkMessage>>,
        >,
    {
        if attempt_budget == 0 {
            return Err("Sumeragi v2 exact-output drive budget must be non-zero".to_owned());
        }
        let mut blocked_sources = BTreeSet::new();
        let mut closest_backpressure_rank: Option<usize> = None;
        let mut attempts = 0usize;
        while !self.fanouts.is_empty() {
            if attempts == attempt_budget {
                return Ok(ExactOutputDriveOutcome::BudgetExhausted {
                    closest_backpressure_rank,
                });
            }
            if let Some((fanout_index, target_index)) = self.next_inactive_reply_target() {
                attempts = attempts
                    .checked_add(1)
                    .expect("bounded exact-output retirement count cannot overflow");
                self.retire_inactive_reply_target(fanout_index, target_index)?;
                continue;
            }
            let Some((fanout_index, target_index)) =
                self.next_schedulable_target(&blocked_sources)?
            else {
                if !self
                    .fanouts
                    .iter()
                    .any(PendingExactFanout::has_dispatchable_target)
                {
                    return Ok(ExactOutputDriveOutcome::Drained);
                }
                if let Some(closest_rank) = closest_backpressure_rank {
                    return Ok(ExactOutputDriveOutcome::Backpressured { closest_rank });
                }
                if self.has_quiescent_fifo_head()? {
                    return Ok(ExactOutputDriveOutcome::Drained);
                }
                return Err(
                    "Sumeragi v2 exact-output scheduler found no per-target FIFO head".to_owned(),
                );
            };
            let inactive_reply = self
                .fanouts
                .get(fanout_index)
                .and_then(|fanout| fanout.targets.get(target_index))
                .is_some_and(|target| {
                    matches!(&target.route, ExactTargetRoute::Reply(route) if !route.is_active())
                });
            if inactive_reply {
                attempts = attempts
                    .checked_add(1)
                    .expect("bounded exact-output retirement count cannot overflow");
                self.retire_inactive_reply_target(fanout_index, target_index)?;
                continue;
            }
            let message_cursor_before = self
                .fanouts
                .get(fanout_index)
                .and_then(|fanout| fanout.targets.get(target_index))
                .ok_or_else(|| "Sumeragi v2 selected sidecar output target disappeared".to_owned())?
                .message_index;
            let message_cursor_after = message_cursor_before
                .checked_add(1)
                .ok_or_else(|| "Sumeragi v2 exact-output message cursor overflowed".to_owned())?;
            let (post, ticket, route, reply_writer_timeout_attempt) = self
                .fanouts
                .get_mut(fanout_index)
                .expect("selected exact fanout must remain present")
                .take_attempt(target_index)
                .expect("selected exact-output target must own an attempt");
            if matches!(&route, ExactTargetRoute::Reply(reply_route) if !reply_route.is_active()) {
                drop(post);
                drop(ticket);
                attempts = attempts
                    .checked_add(1)
                    .expect("bounded exact-output retirement count cannot overflow");
                self.retire_inactive_reply_target(fanout_index, target_index)?;
                continue;
            }
            let attempted_peer = post.peer_id.clone();
            let attempted_source = route.source(&attempted_peer, exact_output_class(&post.data)?);
            let reply_attempt = match &route {
                ExactTargetRoute::Reply(reply_route) => Some((post.clone(), reply_route.clone())),
                ExactTargetRoute::Topology => None,
            };
            let sidecar_reply = match (&post.data, &route) {
                (
                    NetworkMessage::CertifiedMergeSidecar(message),
                    ExactTargetRoute::Reply(reply_route),
                ) => match message.as_ref() {
                    CertifiedMergeSidecarMessage::Chunk(_) => Some((
                        post.clone(),
                        reply_route.clone(),
                        message_cursor_before,
                        message_cursor_after,
                    )),
                    CertifiedMergeSidecarMessage::Request(_)
                    | CertifiedMergeSidecarMessage::Close(_)
                    | CertifiedMergeSidecarMessage::CloseAck(_)
                    | CertifiedMergeSidecarMessage::GenerationHint(_) => None,
                },
                _ => None,
            };
            if sidecar_reply.is_some()
                && self.sidecar_control_units() >= self.sidecar_admission_capacity
            {
                self.fanouts
                    .get_mut(fanout_index)
                    .expect("receipt-backpressured exact fanout must remain present")
                    .retain_returned(target_index, post, ticket)?;
                return Ok(ExactOutputDriveOutcome::ReceiptBackpressured);
            }
            attempts = attempts
                .checked_add(1)
                .expect("bounded exact-output attempt count cannot overflow");
            match attempt(post, ticket, &route, reply_writer_timeout_attempt) {
                Ok(ExactOutputAttemptOutcome::Admitted) => {
                    if reply_attempt.is_some() {
                        return Err(
                            "Sumeragi v2 admitted a reply without its exact writer-flush witness"
                                .to_owned(),
                        );
                    }
                    self.fanouts
                        .get_mut(fanout_index)
                        .expect("admitted exact fanout must remain present")
                        .mark_admitted(target_index)?;
                    self.advance_after_attempt(
                        fanout_index,
                        target_index,
                        Some(&attempted_source),
                    )?;
                }
                Ok(ExactOutputAttemptOutcome::ReplyFlush(flush_ack)) => {
                    if sidecar_reply.is_some() {
                        return Err(
                            "Sumeragi v2 attached an ordinary flush witness to sidecar output"
                                .to_owned(),
                        );
                    }
                    let (canonical_post, reply_route) = reply_attempt.ok_or_else(|| {
                        "Sumeragi v2 attached a reply flush witness to topology output".to_owned()
                    })?;
                    if !flush_ack
                        .identity()
                        .is_bound_to_canonical_reply(&canonical_post)
                        || !flush_ack.identity().is_bound_to_delivery(&reply_route)
                        || flush_ack.identity().reply_writer_timeout_attempt()
                            != reply_writer_timeout_attempt
                    {
                        return Err(
                            "Sumeragi v2 ordinary reply flush changed route, payload, or timeout-attempt identity"
                                .to_owned(),
                        );
                    }
                    let fanout = self
                        .fanouts
                        .get_mut(fanout_index)
                        .expect("flushing exact fanout must remain present");
                    let target = fanout
                        .targets
                        .get_mut(target_index)
                        .expect("flushing exact target must remain present");
                    if target
                        .pending_flush
                        .replace(PendingExactReplyFlush {
                            flush_ack,
                            reply_writer_timeout_attempt,
                            sidecar_admission: None,
                        })
                        .is_some()
                    {
                        return Err(
                            "Sumeragi v2 reply target acquired two writer flushes".to_owned()
                        );
                    }
                    fanout.advance_target_cursor(target_index);
                    self.next_fanout_index = (fanout_index + 1) % self.fanouts.len();
                }
                Ok(ExactOutputAttemptOutcome::SidecarFlush(flush_ack)) => {
                    let (canonical_post, reply_route, message_cursor_before, message_cursor_after) =
                        sidecar_reply.ok_or_else(|| {
                            "Sumeragi v2 attached a sidecar flush witness to non-sidecar output"
                                .to_owned()
                        })?;
                    if flush_ack.identity().reply_writer_timeout_attempt()
                        != reply_writer_timeout_attempt
                    {
                        return Err(
                            "Sumeragi v2 sidecar reply flush changed timeout-attempt identity"
                                .to_owned(),
                        );
                    }
                    let admission = CertifiedMergeSidecarChunkAdmission::from_admitted_reply(
                        &canonical_post,
                        &reply_route,
                        message_cursor_before,
                        message_cursor_after,
                        flush_ack.identity(),
                    )
                    .map_err(|error| error.to_string())?;
                    let fanout = self
                        .fanouts
                        .get_mut(fanout_index)
                        .expect("flushing exact fanout must remain present");
                    let target = fanout
                        .targets
                        .get_mut(target_index)
                        .expect("flushing exact target must remain present");
                    if target
                        .pending_flush
                        .replace(PendingExactReplyFlush {
                            flush_ack,
                            reply_writer_timeout_attempt,
                            sidecar_admission: Some(admission),
                        })
                        .is_some()
                    {
                        return Err(
                            "Sumeragi v2 sidecar target acquired two writer flushes".to_owned()
                        );
                    }
                    if target.message_index != message_cursor_before {
                        return Err(
                            "Sumeragi v2 sidecar cursor advanced before writer flush".to_owned()
                        );
                    }
                    fanout.advance_target_cursor(target_index);
                    self.next_fanout_index = (fanout_index + 1) % self.fanouts.len();
                }
                #[cfg(test)]
                Ok(ExactOutputAttemptOutcome::TestReplyFlushed) => {
                    if reply_attempt.is_none() {
                        return Err(
                            "Sumeragi v2 test attached a synthetic reply flush to topology output"
                                .to_owned(),
                        );
                    }
                    self.fanouts
                        .get_mut(fanout_index)
                        .expect("synthetically flushed exact fanout must remain present")
                        .mark_admitted(target_index)?;
                    self.advance_after_attempt(
                        fanout_index,
                        target_index,
                        Some(&attempted_source),
                    )?;
                }
                Ok(ExactOutputAttemptOutcome::Unavailable) => {
                    if !matches!(&route, ExactTargetRoute::Reply(reply_route)
                        if !reply_route.is_reply_writable())
                    {
                        return Err(
                            "Sumeragi v2 network actor reported an unavailable writable route"
                                .to_owned(),
                        );
                    }
                    self.park_unwritable_reply_target(fanout_index, target_index)?;
                }
                Ok(ExactOutputAttemptOutcome::Retired) => {
                    if !matches!(&route, ExactTargetRoute::Reply(reply_route) if !reply_route.is_active())
                    {
                        return Err(
                            "Sumeragi v2 network actor retired a live exact output route"
                                .to_owned(),
                        );
                    }
                    self.retire_inactive_reply_target(fanout_index, target_index)?;
                }
                Err(NetworkActorAdmissionError::Backpressured {
                    message,
                    ticket,
                    rank,
                }) => {
                    if message.peer_id != attempted_peer {
                        self.fanouts
                            .get_mut(fanout_index)
                            .expect("backpressured exact fanout must remain present")
                            .retain_returned(target_index, message, ticket)?;
                        return Err(
                            "Sumeragi v2 network actor changed an exact output target".to_owned()
                        );
                    }
                    self.fanouts
                        .get_mut(fanout_index)
                        .expect("backpressured exact fanout must remain present")
                        .retain_returned(target_index, message, ticket)?;
                    blocked_sources.insert(attempted_source);
                    closest_backpressure_rank =
                        Some(closest_backpressure_rank.map_or(rank, |current| current.min(rank)));
                    self.advance_after_attempt(fanout_index, target_index, None)?;
                }
                Err(NetworkActorAdmissionError::Closed { message }) => {
                    self.fanouts
                        .get_mut(fanout_index)
                        .expect("closed exact fanout must remain present")
                        .retain_returned(target_index, message, None)?;
                    return Err(
                        "Sumeragi v2 network actor closed during output admission".to_owned()
                    );
                }
                Err(NetworkActorAdmissionError::Rejected {
                    message,
                    reason: NetworkActorAdmissionRejection::InactiveReplyRoute,
                }) => {
                    drop(message);
                    self.retire_inactive_reply_target(fanout_index, target_index)?;
                }
                Err(NetworkActorAdmissionError::Rejected { message, reason }) => {
                    self.fanouts
                        .get_mut(fanout_index)
                        .expect("rejected exact fanout must remain present")
                        .retain_returned(target_index, message, None)?;
                    return Err(format!(
                        "Sumeragi v2 network actor permanently rejected output: {reason:?}"
                    ));
                }
            }
        }
        Ok(ExactOutputDriveOutcome::Drained)
    }
    #[cfg(test)]
    fn drive_with_budget<Attempt>(
        &mut self,
        attempt_budget: usize,
        mut attempt: Attempt,
    ) -> Result<ExactOutputDriveOutcome, String>
    where
        Attempt: FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
            &ExactTargetRoute,
        ) -> Result<(), NetworkActorAdmissionError<Post<NetworkMessage>>>,
    {
        self.drive_with_budget_ack(attempt_budget, |post, ticket, route, _timeout_attempt| {
            attempt(post, ticket, route).map(|()| match route {
                ExactTargetRoute::Topology => ExactOutputAttemptOutcome::Admitted,
                ExactTargetRoute::Reply(_) => ExactOutputAttemptOutcome::TestReplyFlushed,
            })
        })
    }
    fn drive_bounded_with_ack<Attempt>(
        &mut self,
        attempt: Attempt,
    ) -> Result<ExactOutputDriveOutcome, String>
    where
        Attempt: FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
            &ExactTargetRoute,
            u8,
        ) -> Result<
            ExactOutputAttemptOutcome,
            NetworkActorAdmissionError<Post<NetworkMessage>>,
        >,
    {
        self.drive_with_budget_ack(self.drive_attempt_budget, attempt)
    }
    #[cfg(test)]
    fn drive_with<Attempt>(&mut self, attempt: Attempt) -> Result<Option<usize>, String>
    where
        Attempt: FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
            &ExactTargetRoute,
        ) -> Result<(), NetworkActorAdmissionError<Post<NetworkMessage>>>,
    {
        match self.drive_with_budget(usize::MAX, attempt)? {
            ExactOutputDriveOutcome::Drained => Ok(None),
            ExactOutputDriveOutcome::ReceiptBackpressured => Err(
                "unbounded exact-output test drive requires sidecar receipt drainage".to_owned(),
            ),
            ExactOutputDriveOutcome::Backpressured { closest_rank } => Ok(Some(closest_rank)),
            ExactOutputDriveOutcome::BudgetExhausted { .. } => Err(
                "unbounded exact-output test drive unexpectedly exhausted its budget".to_owned(),
            ),
        }
    }
}
fn durable_history_source_covers(
    messages: &[NetworkMessage],
    rollover_claim: &ExactOutputRolloverClaim,
    source_network_id: &iroha_data_model::NetworkId,
    maximum_source_height: wire::Height,
    kura: &Kura,
) -> Result<(), String> {
    let [message] = messages else {
        return Err("Sumeragi v2 durable response claim is not a singleton".to_owned());
    };
    if message.progress_reconstruction() != ProgressReconstruction::Retransmit {
        return Err("Sumeragi v2 durable response is not reconstructible traffic".to_owned());
    }
    let NetworkMessage::SumeragiBlock(envelope) = message else {
        return Err("Sumeragi v2 durable response is not block traffic".to_owned());
    };
    match (rollover_claim, envelope.as_message()) {
        (
            ExactOutputRolloverClaim::DurableCommitCertificateResponse {
                responder: claimed_responder,
                source_height,
                source_context_id,
                ..
            },
            BlockMessage::V2(message),
        ) => {
            let wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) =
                &message.payload
            else {
                return Err("durable CommitQC response changed payload kind".to_owned());
            };
            if *source_height > maximum_source_height {
                return Err("durable CommitQC response belongs to a future height".to_owned());
            }
            let source = kura
                .v2_finality_artifact(*source_height)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| {
                    "durable CommitQC response lost its Kura finality source".to_owned()
                })?;
            if &source.height_context.network_id != source_network_id
                || source.context_id() != *source_context_id
                || response.certificate != source.commit_qc
                || &response.responder != claimed_responder
            {
                return Err(
                    "durable CommitQC response differs from its Kura finality source".to_owned(),
                );
            }
            response
                .validate(&source.height_context)
                .map_err(|error| error.to_string())?;
            Signature::try_from_bytes(&response.signature)
                .map_err(|error| error.to_string())?
                .verify(
                    response.responder.public_key(),
                    &response.signature_preimage(),
                )
                .map_err(|error| error.to_string())
        }
        (
            ExactOutputRolloverClaim::DurableCertifiedBodyResponse {
                responder: claimed_responder,
                source_round,
                source_subject,
                ..
            },
            BlockMessage::V2(message),
        ) => {
            let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &message.payload
            else {
                return Err("durable body response changed payload kind".to_owned());
            };
            if source_round.height > maximum_source_height {
                return Err("durable body response belongs to a future height".to_owned());
            }
            let source = kura
                .v2_finality_artifact(source_round.height)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "durable body response lost its Kura finality source".to_owned())?;
            if &source.height_context.network_id != source_network_id
                || source.context_id() != source_round.context_id
                || source.subject != *source_subject
            {
                return Err(
                    "durable body response differs from its Kura finality source".to_owned(),
                );
            }
            response
                .validate(&source.height_context)
                .map_err(|error| error.to_string())?;
            let responder_index = usize::try_from(response.responder)
                .map_err(|_| "durable body responder index is not representable".to_owned())?;
            let responder = source
                .height_context
                .roster
                .get(responder_index)
                .ok_or_else(|| {
                    "durable body responder is outside the historical roster".to_owned()
                })?;
            if &responder.validator != claimed_responder {
                return Err(
                    "durable body response is not bound to the serving network identity".to_owned(),
                );
            }
            Signature::try_from_bytes(&response.signature)
                .map_err(|error| error.to_string())?
                .verify(
                    responder.validator.public_key(),
                    &response.signature_preimage(),
                )
                .map_err(|error| error.to_string())?;
            let block_height = usize::try_from(source_round.height)
                .ok()
                .and_then(NonZeroUsize::new)
                .ok_or_else(|| "durable body source height is not representable".to_owned())?;
            let block = kura
                .get_block(block_height)
                .ok_or_else(|| "durable body response lost its canonical Kura block".to_owned())?;
            let proposal = block.canonical_resultless_proposal();
            let canonical_wire = proposal.encode_wire().map_err(|error| error.to_string())?;
            if block.hash() != source_subject.block_hash
                || canonical_wire != response.body
                || Hash::new(&canonical_wire) != source_subject.payload_hash
            {
                return Err("durable body response differs from its canonical Kura body".to_owned());
            }
            let (manifest, _) = encode_payload(
                &source.height_context,
                *source_round,
                *source_subject,
                &canonical_wire,
            )
            .map_err(|error| error.to_string())?
            .into_parts();
            if manifest != response.manifest {
                return Err("durable body response manifest is not Kura-reconstructible".to_owned());
            }
            Ok(())
        }
        (
            ExactOutputRolloverClaim::DurableLaneCertificateResponse {
                lane_id,
                lane_block_height,
                proposal_height,
                proposal_hash,
                ..
            },
            BlockMessage::LaneBlockCertificate(certificate),
        ) => {
            if *proposal_height > maximum_source_height {
                return Err("durable lane certificate belongs to a future height".to_owned());
            }
            let source = kura
                .read_certified_lane_block_artifact(*lane_id, *lane_block_height)
                .ok_or_else(|| {
                    "durable lane certificate lost its certified Kura source".to_owned()
                })?;
            if source.proposal.descriptor.proposal_height != *proposal_height
                || source.proposal.proposal_hash != *proposal_hash
                || certificate.proposal != source.proposal
                || certificate.prepare_qc != source.prepare_qc
                || certificate.commit_qc != source.commit_qc
            {
                return Err(
                    "durable lane certificate differs from its certified Kura source".to_owned(),
                );
            }
            Ok(())
        }
        (
            ExactOutputRolloverClaim::HistoricalAutonomousLaneCertification {
                source_height,
                lane_id,
                lane_block_height,
                proposal_hash,
                message_hash,
                ..
            },
            message,
        ) => {
            if *source_height >= maximum_source_height || HashOf::new(message) != *message_hash {
                return Err(
                    "historical autonomous certification has an invalid source height or hash"
                        .to_owned(),
                );
            }
            let records = kura
                .historical_autonomous_lane_recovery_records_bounded(
                    crate::kura::HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
                )
                .map_err(|error| error.to_string())?;
            let record = records
                .into_iter()
                .find(|record| {
                    let proposal = &record.payload.origin_proposal;
                    proposal.descriptor.proposal_height == *source_height
                        && proposal.descriptor.lane_id == *lane_id
                        && proposal.descriptor.lane_block_height == *lane_block_height
                        && proposal.proposal_hash == *proposal_hash
                })
                .ok_or_else(|| {
                    "historical autonomous certification lost its immutable Kura record".to_owned()
                })?;
            kura.validate_historical_autonomous_lane_recovery_record_dependencies(&record)
                .map_err(|error| error.to_string())?;
            let proposal = &record.payload.origin_proposal;
            match message {
                BlockMessage::LaneBlockProposal(candidate) if candidate == proposal => Ok(()),
                BlockMessage::LaneBlockVote(vote)
                    if vote.body == proposal.vote_body(vote.body.phase)
                        && proposal.descriptor.validator_set.contains(&vote.signer) =>
                {
                    vote.validate_ingress(vote.body.phase)
                        .map_err(|error| error.to_string())
                }
                BlockMessage::LaneBlockQc(qc)
                    if qc.body == proposal.vote_body(qc.body.phase)
                        && qc.validator_set == proposal.descriptor.validator_set =>
                {
                    let pops = qc
                        .validator_set
                        .iter()
                        .zip(&record.validator_pops)
                        .enumerate()
                        .filter(|(index, _)| {
                            qc.signers_bitmap
                                .get(index / 8)
                                .is_some_and(|byte| byte & (1_u8 << (index % 8)) != 0)
                        })
                        .map(|(_, (peer, pop))| (peer.public_key().clone(), pop.clone()))
                        .collect();
                    crate::lane_consensus::validate_lane_block_qc_aggregate(qc, &pops)
                        .map_err(|error| error.to_string())
                }
                _ => Err(
                    "historical autonomous certification differs from its immutable proposal"
                        .to_owned(),
                ),
            }
        }
        (
            ExactOutputRolloverClaim::HistoricalLaneRecoveryResponse {
                request_hash,
                response_hash,
                ..
            },
            BlockMessage::LaneHistoricalRecoveryResponse(response),
        ) => {
            if response.request_hash != *request_hash
                || HashOf::new(response.as_ref()) != *response_hash
                || response.version != super::message::LANE_HISTORICAL_RECOVERY_VERSION_V4
            {
                return Err(
                    "historical lane recovery response changed its exact request binding"
                        .to_owned(),
                );
            }
            match &response.payload {
                LaneHistoricalRecoveryPayloadV1::CanonicalBlock {
                    block,
                    finality_artifact,
                } => {
                    let height = block.header().height().get();
                    if height > maximum_source_height {
                        return Err(
                            "historical canonical-body response belongs to a future height"
                                .to_owned(),
                        );
                    }
                    let source = kura
                        .v2_finality_artifact(height)
                        .map_err(|error| error.to_string())?
                        .ok_or_else(|| {
                            "historical canonical-body response lost its finality source".to_owned()
                        })?;
                    if &source.height_context.network_id != source_network_id
                        || source != *finality_artifact
                        || source.validate_for_header(&block.header()).is_err()
                        || source.verify().is_err()
                    {
                        return Err(
                            "historical canonical-body response differs from Kura finality"
                                .to_owned(),
                        );
                    }
                    let height = usize::try_from(height)
                        .ok()
                        .and_then(NonZeroUsize::new)
                        .ok_or_else(|| {
                            "historical canonical-body height is not representable".to_owned()
                        })?;
                    if kura.get_block(height).as_deref() != Some(block) {
                        return Err(
                            "historical canonical-body response differs from Kura body".to_owned()
                        );
                    }
                    Ok(())
                }
                LaneHistoricalRecoveryPayloadV1::AutonomousPayload {
                    payload,
                    prepare_qc,
                    commit_qc,
                } => {
                    let descriptor = &payload.origin_proposal.descriptor;
                    if descriptor.proposal_height > maximum_source_height {
                        return Err(
                            "historical autonomous response belongs to a future height".to_owned()
                        );
                    }
                    let certified = kura
                        .read_certified_lane_block_artifact(
                            descriptor.lane_id,
                            descriptor.lane_block_height,
                        )
                        .ok_or_else(|| {
                            "historical autonomous response lost its certified Kura source"
                                .to_owned()
                        })?;
                    if certified.proposal != payload.origin_proposal
                        || certified.prepare_qc != *prepare_qc
                        || certified.commit_qc != *commit_qc
                    {
                        return Err(
                            "historical autonomous response differs from certified Kura evidence"
                                .to_owned(),
                        );
                    }
                    let expected_epoch = payload.epoch;
                    let (durable_payload, _) = kura
                        .current_autonomous_lane_payload(
                            descriptor.lane_id,
                            descriptor.lane_block_height,
                            payload.network_id,
                            expected_epoch,
                        )
                        .ok_or_else(|| {
                            "historical autonomous response lost its payload sidecar".to_owned()
                        })?;
                    let durable_availability = kura
                        .read_autonomous_lane_block_artifact(
                            descriptor.lane_id,
                            descriptor.lane_block_height,
                            payload.network_id,
                            expected_epoch,
                        )
                        .and_then(|artifact| artifact.availability_certificate);
                    if durable_payload != *payload
                        || durable_availability
                            .is_none_or(|certificate| certificate.certificate != *prepare_qc)
                    {
                        return Err(
                            "historical autonomous response differs from its READY sidecar"
                                .to_owned(),
                        );
                    }
                    Ok(())
                }
                LaneHistoricalRecoveryPayloadV1::CanonicalExecutedBlockChunk {
                    finality_artifact,
                    wire_len,
                    chunk_index,
                    chunk_count,
                    bytes,
                } => {
                    let height = finality_artifact.height;
                    if height == 0 || height > maximum_source_height {
                        return Err(
                            "historical canonical executed-block chunk belongs to an invalid or future height"
                                .to_owned(),
                        );
                    }
                    let source = kura
                        .v2_finality_artifact(height)
                        .map_err(|error| error.to_string())?
                        .ok_or_else(|| {
                            "historical canonical executed-block chunk lost its finality source"
                                .to_owned()
                        })?;
                    let height_index = usize::try_from(height)
                        .ok()
                        .and_then(NonZeroUsize::new)
                        .ok_or_else(|| {
                            "historical canonical executed-block height is not representable"
                                .to_owned()
                        })?;
                    let block = kura
                        .get_block_without_merge_sidecar(height_index)
                        .ok_or_else(|| {
                            "historical canonical executed-block chunk lost its Kura body"
                                .to_owned()
                        })?;
                    if &source.height_context.network_id != source_network_id
                        || source != *finality_artifact
                        || source.verify().is_err()
                        || source.validate_for_header(&block.header()).is_err()
                        || block.header().height().get() != height
                        || block.hash() != source.block_hash
                        || source.commit_qc.execution_commitment.validate().is_err()
                        || !block.executed_block_wire_hash().is_ok_and(|hash| {
                            hash == source
                                .commit_qc
                                .execution_commitment
                                .executed_block_wire_hash
                        })
                    {
                        return Err(
                            "historical canonical executed-block chunk differs from Kura finality"
                                .to_owned(),
                        );
                    }
                    let canonical_wire = block.encode_wire().map_err(|error| error.to_string())?;
                    let expected_wire_len =
                        u64::try_from(canonical_wire.len()).map_err(|error| error.to_string())?;
                    let expected_chunk_count = canonical_wire
                        .len()
                        .div_ceil(crate::merge_sidecar::MAX_CERTIFIED_MERGE_CHUNK_BYTES);
                    let expected_chunk_count_u32 =
                        u32::try_from(expected_chunk_count).map_err(|error| error.to_string())?;
                    let chunk_index_usize =
                        usize::try_from(*chunk_index).map_err(|error| error.to_string())?;
                    let start = chunk_index_usize
                        .checked_mul(crate::merge_sidecar::MAX_CERTIFIED_MERGE_CHUNK_BYTES)
                        .ok_or_else(|| {
                            "historical canonical executed-block chunk offset overflow".to_owned()
                        })?;
                    let end = start
                        .saturating_add(crate::merge_sidecar::MAX_CERTIFIED_MERGE_CHUNK_BYTES)
                        .min(canonical_wire.len());
                    if canonical_wire.is_empty()
                        || expected_wire_len > crate::kura::STRICT_INIT_MAX_BLOCK_BYTES
                        || *wire_len != expected_wire_len
                        || expected_chunk_count == 0
                        || *chunk_count != expected_chunk_count_u32
                        || chunk_index_usize >= expected_chunk_count
                        || bytes.as_slice() != &canonical_wire[start..end]
                    {
                        return Err(
                            "historical canonical executed-block chunk differs from its exact Kura wire"
                                .to_owned(),
                        );
                    }
                    Ok(())
                }
            }
        }
        _ => Err("Sumeragi v2 durable response claim changed output kind".to_owned()),
    }
}
include!("v2_worker/autonomous_lane_output_reconstruction.rs");
include!("v2_worker/kura_replica_advert_refresh.rs");
/// Concrete effect services used by the live v2 height runner.
pub(crate) struct ProductionV2Services {
    context: wire::HeightContext,
    validator_set_pops: Vec<Vec<u8>>,
    state: Arc<crate::state::State>,
    local_peer: PeerId,
    local_validator: Option<wire::ValidatorIndex>,
    key_pair: KeyPair,
    network: IrohaNetwork,
    kura: Arc<Kura>,
    chunk_root: PathBuf,
    io: Option<V2IoHandle>,
    lifecycle_body_store_identity: Option<V2BodyStoreInstanceIdentity>,
    lifecycle_payload_store_identity: Option<CertifiedServePayloadStoreInstanceIdentity>,
    fetches: BTreeMap<EffectWorkId, FetchSession>,
    fetch_by_manifest: BTreeMap<HashOf<wire::PayloadManifest>, EffectWorkId>,
    orphan_chunks: BTreeMap<HashOf<wire::PayloadManifest>, VecDeque<BufferedPayloadChunk>>,
    orphan_chunk_count: usize,
    orphan_chunk_bytes: u64,
    orphan_lifecycle_sweep_cursor: Option<OrphanPayloadLifecycleSweepCursor>,
    max_orphan_chunks: usize,
    max_orphan_chunk_bytes: u64,
    max_merge_sidecar_deferrals: usize,
    local_completions: VecDeque<LocalCompletion>,
    held_io_completion: Option<V2IoCompletion>,
    next_completion_source: CompletionSource,
    locked_candidate_acquisition: Option<LockedCandidateAcquisition>,
    next_locked_candidate_acquisition_id: u64,
    proposal_work_retired: bool,
    prepared_candidates: VecDeque<PreparedCandidateBody>,
    validation_rejections: VecDeque<RejectedCandidateBody>,
    merge_sidecar_deferrals: VecDeque<DeferredMergeSidecarWork>,
    outbound_chunks: BTreeMap<HashOf<wire::PayloadManifest>, RetainedOutboundPayload>,
    fast_path_proposals: BTreeSet<wire::ConsensusRound>,
    pending_exact_output: Mutex<PendingExactOutput>,
    /// Process-lifetime proactive refresh owner shared across immutable height
    /// services. Its retained Kura token is not pending exact output and never
    /// participates in finality sealing.
    kura_replica_advert_refresh: Arc<KuraReplicaAdvertRefreshOwner>,
    exact_output_handoff_owner: DurableExactOutputServiceOwner,
    #[cfg(test)]
    exact_output_admission_hook: Option<Mutex<ExactOutputAdmissionHook>>,
    active_tag: EventTag,
    last_status: Option<EffectExecutorStatus>,
    fatal_reason: Option<String>,
    output_guard: Arc<ConsensusOutputGuard>,
    leader_wire_ingress: Arc<FairV2Ingress>,
    leader_wire_recovery_authority: super::serviced_candidate_store::LeaderWireRecoveryAuthority,
    clean_teardown: bool,
}
/// Private move-only permit for unpacking one WAL/registry signed Broadcast.
pub(in crate::sumeragi) struct RecoveredLifecycleSignBroadcastOutputPermitV1 {
    _linearity: RecoveredLifecycleSignBroadcastOutputPermitLinearityV1,
}
struct RecoveredLifecycleSignBroadcastOutputPermitLinearityV1;
impl Drop for RecoveredLifecycleSignBroadcastOutputPermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleSignBroadcastOutputPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleSignBroadcastOutputPermitLinearityV1,
        }
    }
}
/// Private one-shot next-Vote lookup permit enforcing worker/store identity.
pub(in crate::sumeragi) struct RecoveredLifecycleNextVoteBodyExecutorPermitV1 {
    _linearity: RecoveredLifecycleNextVoteBodyExecutorPermitLinearityV1,
    context: wire::HeightContext,
    requester: PeerId,
    output_guard: Arc<ConsensusOutputGuard>,
    body_store_identity: V2BodyStoreInstanceIdentity,
}
struct RecoveredLifecycleNextVoteBodyExecutorPermitLinearityV1;
impl Drop for RecoveredLifecycleNextVoteBodyExecutorPermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleNextVoteBodyExecutorPermitV1 {
    fn new(
        context: wire::HeightContext,
        requester: PeerId,
        output_guard: Arc<ConsensusOutputGuard>,
        body_store_identity: V2BodyStoreInstanceIdentity,
    ) -> Self {
        Self {
            _linearity: RecoveredLifecycleNextVoteBodyExecutorPermitLinearityV1,
            context,
            requester,
            output_guard,
            body_store_identity,
        }
    }
    /// Consume only against the same executor/store owner joined by the service.
    pub(in crate::sumeragi) fn consume_for_executor(
        self,
        context: &wire::HeightContext,
        requester: &PeerId,
        output_guard: &Arc<ConsensusOutputGuard>,
        body_store_identity: &V2BodyStoreInstanceIdentity,
    ) -> Option<V2BodyStoreInstanceIdentity> {
        (self.context == *context
            && self.requester == *requester
            && Arc::ptr_eq(&self.output_guard, output_guard)
            && self.body_store_identity.same_instance(body_store_identity))
        .then_some(self.body_store_identity)
    }
}
/// Private permit consuming an adapter-sealed Proposal control/payload pair
/// behind one exact-output reservation.
pub(in crate::sumeragi) struct RecoveredLifecycleProposalExactOutputPermitV1 {
    _linearity: RecoveredLifecycleProposalExactOutputPermitLinearityV1,
}
struct RecoveredLifecycleProposalExactOutputPermitLinearityV1;
impl Drop for RecoveredLifecycleProposalExactOutputPermitLinearityV1 {
    fn drop(&mut self) {}
}
impl RecoveredLifecycleProposalExactOutputPermitV1 {
    fn new() -> Self {
        Self {
            _linearity: RecoveredLifecycleProposalExactOutputPermitLinearityV1,
        }
    }
}
/// Result of reserving exact output for a recovered signed Broadcast.
#[allow(variant_size_differences, clippy::large_enum_variant)]
pub(in crate::sumeragi) enum RecoveredLifecycleSignBroadcastOutputCaptureV1<'service> {
    /// The bounded corridor cannot retain this fanout yet; nothing changed.
    Unavailable,
    /// The exact corridor mutex and fail-stop operation remain retained.
    Reserved(RecoveredLifecycleSignBroadcastOutputReservationV1<'service>),
}
/// Borrow-bound exact-output reservation for one durable recovered Broadcast.
///
/// Dropping the armed reservation fail-stops. The caller must first park the
/// volatile claim while leaving LedgerV1 Ready, then commit the fanout.
#[must_use = "recovered signed Broadcast output must enter its exact corridor"]
pub(in crate::sumeragi) struct RecoveredLifecycleSignBroadcastOutputReservationV1<'service> {
    operation: Option<ConsensusFailStopOperation<'service>>,
    pending: Option<std::sync::MutexGuard<'service, PendingExactOutput>>,
    output: Option<RecoveredLifecycleSignBroadcastPreparedOutputV1>,
}
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum RecoveredLifecycleSignBroadcastPreparedOutputV1 {
    Single(Option<PendingExactFanout>),
    Proposal(PendingExactOutputBatchPlan),
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredLifecycleSignBroadcastOutputReservationV1<'_> {
    /// Publish the preflighted fanout in the assertion-only post-fsync tail.
    pub(in crate::sumeragi) fn commit_after_publication(mut self) {
        let mut pending = self
            .pending
            .take()
            .expect("recovered Sign output reservation retains its corridor mutex");
        let operation = self
            .operation
            .take()
            .expect("recovered Sign output commit retains its fail-stop operation");
        match self
            .output
            .take()
            .expect("recovered Sign output retains its exact publication")
        {
            RecoveredLifecycleSignBroadcastPreparedOutputV1::Single(fanout) => {
                if let Some(fanout) = fanout {
                    assert_eq!(
                        pending.enqueue(fanout),
                        Ok(ExactFanoutOwnership::Owned),
                        "preflighted recovered Sign fanout must enter exact-output ownership"
                    );
                }
            }
            RecoveredLifecycleSignBroadcastPreparedOutputV1::Proposal(batch) => {
                pending.commit_atomic_fanout_batch(batch);
            }
        }
        drop(pending);
        operation.complete();
    }
}
/// Result of atomically reserving Proposal control and payload fanouts.
#[allow(variant_size_differences, clippy::large_enum_variant)]
pub(in crate::sumeragi) enum RecoveredLifecycleProposalExactOutputCaptureV1<'service> {
    /// Aggregate ownership does not fit; the corridor remains unchanged and
    /// the exact authority is returned for a later bounded retry.
    Unavailable(super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1),
    /// Both fanouts remain behind one mutex and fail-stop operation.
    Reserved(RecoveredLifecycleProposalExactOutputReservationV1<'service>),
}
/// Borrow-bound atomic Proposal output reservation.
///
/// Dropping while armed fail-stops output. Every recoverable prepublication
/// path must consume [`Self::abort_before_publication`].
#[must_use = "recovered Proposal output must commit atomically or use its typed abort"]
pub(in crate::sumeragi) struct RecoveredLifecycleProposalExactOutputReservationV1<'service> {
    operation: Option<ConsensusFailStopOperation<'service>>,
    pending: Option<std::sync::MutexGuard<'service, PendingExactOutput>>,
    batch: Option<PendingExactOutputBatchPlan>,
    authority: Option<super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1>,
    wal_append: RecoveredLifecycleProposalPrepareWalAppendSealV1,
}

/// Identity seal created after a Proposal control/chunk batch owns capacity;
/// its borrow prevents WAL append from bypassing the reservation.
struct RecoveredLifecycleProposalPrepareWalAppendSealV1 {
    dispatch_key: super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
    body_store_identity: V2BodyStoreInstanceIdentity,
    output_guard: Arc<ConsensusOutputGuard>,
    attempted: bool,
}

/// Borrow-bound proof that one exact Proposal batch remains reserved.
#[must_use = "the Proposal WAL append permit must remain tied to its output reservation"]
pub(in crate::sumeragi) struct RecoveredLifecycleProposalPrepareWalAppendPermitV1<'reservation> {
    seal: &'reservation mut RecoveredLifecycleProposalPrepareWalAppendSealV1,
}

impl RecoveredLifecycleProposalPrepareWalAppendPermitV1<'_> {
    /// Compare the preview owner without exposing any reservation constituent.
    pub(in crate::sumeragi) fn authorizes(
        &self,
        dispatch_key: super::v2_lifecycle_coordinator::RecoveredLifecycleSignDispatchKeyV1,
        body_store_identity: &V2BodyStoreInstanceIdentity,
        output_guard: &Arc<ConsensusOutputGuard>,
    ) -> bool {
        !self.seal.attempted
            && self.seal.dispatch_key == dispatch_key
            && self
                .seal
                .body_store_identity
                .same_instance(body_store_identity)
            && Arc::ptr_eq(&self.seal.output_guard, output_guard)
    }

    /// Irreversibly cross the retry boundary before attempting the WAL append.
    pub(in crate::sumeragi) fn cross_wal_attempt_boundary(self) {
        self.seal.attempted = true;
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl RecoveredLifecycleProposalExactOutputReservationV1<'_> {
    /// Borrow the sole initial-Proposal WAL permit while this batch is armed.
    pub(in crate::sumeragi) fn prepare_wal_append_permit(
        &mut self,
    ) -> Option<RecoveredLifecycleProposalPrepareWalAppendPermitV1<'_>> {
        (self.operation.is_some()
            && self.pending.is_some()
            && self.batch.is_some()
            && self.authority.is_some()
            && !self.wal_append.attempted)
            .then_some(RecoveredLifecycleProposalPrepareWalAppendPermitV1 {
                seal: &mut self.wal_append,
            })
    }

    /// Release an unchanged aggregate reservation before durable publication.
    pub(in crate::sumeragi) fn abort_before_publication(
        mut self,
    ) -> super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1 {
        assert!(
            !self.wal_append.attempted,
            "an attempted Proposal WAL cut cannot return to the prepublication retry boundary"
        );
        drop(self.pending.take());
        drop(self.batch.take());
        self.operation
            .take()
            .expect("armed recovered Proposal output retains its fail-stop operation")
            .complete();
        self.authority
            .take()
            .expect("armed recovered Proposal output retains its retry authority")
    }
    /// Install both preflighted fanouts in one assertion-only publication tail.
    pub(in crate::sumeragi) fn commit_after_publication(mut self) {
        let mut pending = self
            .pending
            .take()
            .expect("recovered Proposal output retains its corridor mutex");
        // Take this after the mutex guard: reverse local-drop order closes
        // output before unlocking the corridor if any assertion below unwinds.
        let operation = self
            .operation
            .take()
            .expect("recovered Proposal output retains its fail-stop operation");
        let batch = self
            .batch
            .take()
            .expect("recovered Proposal output retains its aggregate batch");
        let authority = self
            .authority
            .take()
            .expect("recovered Proposal output commit retains its exact authority");
        pending.commit_atomic_fanout_batch(batch);
        drop(pending);
        drop(authority);
        operation.complete();
    }
}
/// Borrow-bound exact-output reservation retained before coordinator claim.
///
/// Preencoding, topology construction, rollover validation, and `can_enqueue`
/// all precede scheduler planning. Dropping an armed reservation closes output;
/// recoverable pre-claim failures must consume [`Self::abort_before_claim`].
#[must_use = "exact recovered Fetch output must commit or use its typed pre-claim abort"]
pub(in crate::sumeragi) struct RecoveredDecisionFetchExactOutputReservationV1<'service> {
    operation: Option<ConsensusFailStopOperation<'service>>,
    pending: Option<std::sync::MutexGuard<'service, PendingExactOutput>>,
    fanout: Option<PendingExactFanout>,
}
impl RecoveredDecisionFetchExactOutputReservationV1<'_> {
    /// Test-only release of an unchanged pre-claim reservation.
    #[cfg(test)]
    pub(in crate::sumeragi) fn abort_before_claim(mut self) {
        drop(self.pending.take());
        self.operation
            .take()
            .expect("armed recovered Fetch output retains its fail-stop operation")
            .complete();
    }
    /// Publish the preflighted fanout in the assertion-only post-arming tail.
    pub(in crate::sumeragi) fn commit(mut self) {
        let mut pending = self
            .pending
            .take()
            .expect("recovered Fetch output reservation retains its corridor mutex");
        // Take this after the mutex guard so unwinding closes output before
        // releasing the exact-output corridor.
        let operation = self
            .operation
            .take()
            .expect("recovered Fetch output commit retains its fail-stop operation");
        if let Some(fanout) = self.fanout.take() {
            assert_eq!(
                pending.enqueue(fanout),
                Ok(ExactFanoutOwnership::Owned),
                "preflighted recovered Fetch fanout must enter exact-output ownership"
            );
        }
        drop(pending);
        operation.complete();
    }
}
fn maximum_orphan_chunk_bytes(layout: wire::DataAvailabilityLayout) -> u64 {
    u64::from(layout.max_chunk_count)
        .saturating_mul(u64::from(layout.chunk_size_bytes))
        .min(wire::MAX_DA_ENCODED_PAYLOAD_BYTES)
}
impl ProductionV2Services {
    /// Reserve output after a recovered Broadcast rejoins its LedgerV1 row,
    /// retaining that durable row as crash-recovery debt.
    pub(in crate::sumeragi) fn capture_recovered_lifecycle_signed_broadcast_refanout(
        &self,
        authority: super::v2_lifecycle_coordinator::RecoveredLifecycleSignedBroadcastOutputAuthorityV1,
    ) -> Result<RecoveredLifecycleSignBroadcastOutputCaptureV1<'_>, String> {
        let (context_id, height, message, cold_proposal_output) =
            authority.consume_for_service(RecoveredLifecycleSignBroadcastOutputPermitV1::new());
        if context_id != self.context.id()
            || height != self.context.height
            || self.exact_output_handoff_owner.is_sealed()
        {
            return Err(
                "recovered signed Broadcast refanout belongs to another service cut".to_owned(),
            );
        }
        if let Some(output) = cold_proposal_output {
            self.capture_recovered_lifecycle_cold_proposal_message(message, output)
        } else {
            self.capture_recovered_lifecycle_signed_broadcast_message(message)
        }
    }
    fn capture_recovered_lifecycle_signed_broadcast_message(
        &self,
        message: wire::ConsensusMessageV2,
    ) -> Result<RecoveredLifecycleSignBroadcastOutputCaptureV1<'_>, String> {
        if !matches!(
            &message.payload,
            wire::ConsensusMessageV2Payload::Vote(_)
                | wire::ConsensusMessageV2Payload::TimeoutVote(_)
        ) || self.exact_output_handoff_owner.is_sealed()
        {
            return Err(
                "recovered Sign Broadcast is outside the exact single-child service cut".to_owned(),
            );
        }
        message
            .validate_version()
            .map_err(|error| error.to_string())?;
        let encoded = Self::preencode_v2_network_message(message)?;
        let fanout = PendingExactFanout::claimed(
            vec![encoded],
            self.remote_voters(),
            ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
        )?;
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "recovered Sign exact output requires restart".to_owned())?;
        let pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            drop(operation);
            drop(pending);
            return Err("recovered Sign exact output sealed during capture".to_owned());
        }
        if let Some(fanout) = fanout.as_ref() {
            let available = match pending.can_enqueue(fanout) {
                Ok(available) => available,
                Err(error) => {
                    drop(operation);
                    drop(pending);
                    return Err(error);
                }
            };
            if !available {
                drop(pending);
                operation.complete();
                return Ok(RecoveredLifecycleSignBroadcastOutputCaptureV1::Unavailable);
            }
        }
        Ok(RecoveredLifecycleSignBroadcastOutputCaptureV1::Reserved(
            RecoveredLifecycleSignBroadcastOutputReservationV1 {
                operation: Some(operation),
                pending: Some(pending),
                output: Some(RecoveredLifecycleSignBroadcastPreparedOutputV1::Single(
                    fanout,
                )),
            },
        ))
    }
    #[allow(clippy::too_many_lines)]
    fn capture_recovered_lifecycle_cold_proposal_message(
        &self,
        message: wire::ConsensusMessageV2,
        output: super::v2::RecoveredLifecycleColdProposalOutputV1,
    ) -> Result<RecoveredLifecycleSignBroadcastOutputCaptureV1<'_>, String> {
        if self.proposal_work_retired || self.exact_output_handoff_owner.is_sealed() {
            return Err(
                "cold recovered Proposal output is outside the live service cut".to_owned(),
            );
        }
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload else {
            return Err(
                "cold recovered Proposal output lost its signed control message".to_owned(),
            );
        };
        let (payload, body_store_identity) =
            output.consume_for_service(RecoveredLifecycleProposalExactOutputPermitV1::new());
        if self.io.is_none()
            || self.local_validator != Some(proposal.proposer)
            || self.local_peer.public_key() != self.key_pair.public_key()
            || self
                .lifecycle_body_store_identity
                .as_ref()
                .is_none_or(|identity| !identity.same_instance(&body_store_identity))
            || proposal.manifest != *payload.manifest()
        {
            return Err("cold recovered Proposal output belongs to another service cut".to_owned());
        }
        message
            .validate_version()
            .map_err(|error| error.to_string())?;
        proposal
            .validate(&self.context)
            .map_err(|error| error.to_string())?;
        let (manifest, chunks) = payload.into_parts();
        manifest
            .validate(&self.context)
            .map_err(|error| error.to_string())?;
        let manifest_hash = HashOf::new(&manifest);
        let sender = proposal.proposer;
        let mut chunk_messages = Vec::with_capacity(chunks.len());
        for (index, bytes) in chunks.into_iter().enumerate() {
            let mut chunk = wire::PayloadChunk {
                manifest_hash,
                index: u32::try_from(index)
                    .map_err(|_| "cold recovered Proposal chunk index overflowed".to_owned())?,
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
            chunk_messages.push(Self::preencode_v2_network_message(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(chunk)),
            )?);
        }
        let peers = self.remote_voters();
        let control = PendingExactFanout::claimed(
            vec![Self::preencode_v2_network_message(message)?],
            peers.clone(),
            ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
        )?;
        let chunks = PendingExactFanout::claimed(
            chunk_messages,
            peers,
            ExactOutputRolloverClaim::PayloadChunks {
                scope: self.exact_output_scope(),
                manifest,
            },
        )?;
        let fanouts = control.into_iter().chain(chunks).collect::<Vec<_>>();
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "cold recovered Proposal exact output requires restart".to_owned())?;
        let pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            drop(operation);
            drop(pending);
            return Err("cold recovered Proposal output sealed during capture".to_owned());
        }
        let batch = match pending.prepare_atomic_fanout_batch(fanouts) {
            Ok(batch) => batch,
            Err(error) => {
                drop(operation);
                drop(pending);
                return Err(error);
            }
        };
        let Some(batch) = batch else {
            drop(pending);
            operation.complete();
            return Ok(RecoveredLifecycleSignBroadcastOutputCaptureV1::Unavailable);
        };
        Ok(RecoveredLifecycleSignBroadcastOutputCaptureV1::Reserved(
            RecoveredLifecycleSignBroadcastOutputReservationV1 {
                operation: Some(operation),
                pending: Some(pending),
                output: Some(RecoveredLifecycleSignBroadcastPreparedOutputV1::Proposal(
                    batch,
                )),
            },
        ))
    }
    /// Atomically reserve a recovered Proposal and all chunks under one corridor
    /// lock; capacity failure leaves every FIFO/index/fanout unchanged.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::too_many_lines)]
    pub(in crate::sumeragi) fn capture_recovered_lifecycle_proposal_exact_output(
        &self,
        authority: super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1,
    ) -> Result<RecoveredLifecycleProposalExactOutputCaptureV1<'_>, String> {
        if self.proposal_work_retired {
            return Err("recovered Proposal output is terminal after Decision".to_owned());
        }
        let (dispatch_key, tag, message, payload, body_store_identity, authority_output_guard) =
            authority.consume_for_service(RecoveredLifecycleProposalExactOutputPermitV1::new());
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload else {
            return Err("recovered Proposal output lost its signed control message".to_owned());
        };
        if !dispatch_key.matches_height_context(&self.context)
            || tag != self.active_tag
            || proposal.round.context_id != self.context.id()
            || proposal.round.height != self.context.height
            || proposal.round.view != tag.view()
            || self.local_validator != Some(proposal.proposer)
            || self.local_peer.public_key() != self.key_pair.public_key()
            || proposal.manifest != *payload.manifest()
            || self.io.is_none()
            || self
                .lifecycle_body_store_identity
                .as_ref()
                .is_none_or(|identity| !identity.same_instance(&body_store_identity))
            || !Arc::ptr_eq(&self.output_guard, &authority_output_guard)
            || self.exact_output_handoff_owner.is_sealed()
        {
            return Err("recovered Proposal output belongs to another service cut".to_owned());
        }
        message
            .validate_version()
            .map_err(|error| error.to_string())?;
        proposal
            .validate(&self.context)
            .map_err(|error| error.to_string())?;
        let wal_append = RecoveredLifecycleProposalPrepareWalAppendSealV1 {
            dispatch_key,
            body_store_identity: body_store_identity.clone(),
            output_guard: Arc::clone(&authority_output_guard),
            attempted: false,
        };
        let retry_authority =
            super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1::from_service_retry(
                RecoveredLifecycleProposalExactOutputPermitV1::new(),
                &self.context,
                dispatch_key,
                tag,
                message.clone(),
                payload.clone(),
                body_store_identity,
                authority_output_guard,
            )
            .ok_or_else(|| {
                "recovered Proposal output could not retain its exact retry authority".to_owned()
            })?;
        let (manifest, chunks) = payload.into_parts();
        manifest
            .validate(&self.context)
            .map_err(|error| error.to_string())?;
        let manifest_hash = HashOf::new(&manifest);
        let sender = proposal.proposer;
        let mut chunk_messages = Vec::with_capacity(chunks.len());
        for (index, bytes) in chunks.into_iter().enumerate() {
            let mut chunk = wire::PayloadChunk {
                manifest_hash,
                index: u32::try_from(index)
                    .map_err(|_| "recovered Proposal chunk index overflowed".to_owned())?,
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
            chunk_messages.push(Self::preencode_v2_network_message(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(chunk)),
            )?);
        }
        let peers = self.remote_voters();
        let control = PendingExactFanout::claimed(
            vec![Self::preencode_v2_network_message(message)?],
            peers.clone(),
            ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
        )?;
        let chunks = PendingExactFanout::claimed(
            chunk_messages,
            peers,
            ExactOutputRolloverClaim::PayloadChunks {
                scope: self.exact_output_scope(),
                manifest,
            },
        )?;
        let fanouts = control.into_iter().chain(chunks).collect::<Vec<_>>();
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "recovered Proposal exact output requires restart".to_owned())?;
        let pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            // Activate fail-stop while the corridor remains locked; releasing
            // the mutex first would leave a brief open-admission window.
            drop(operation);
            drop(pending);
            return Err("recovered Proposal exact output sealed during capture".to_owned());
        }
        let batch = match pending.prepare_atomic_fanout_batch(fanouts) {
            Ok(batch) => batch,
            Err(error) => {
                drop(operation);
                drop(pending);
                return Err(error);
            }
        };
        let Some(batch) = batch else {
            drop(pending);
            operation.complete();
            return Ok(RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(
                retry_authority,
            ));
        };
        Ok(RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(
            RecoveredLifecycleProposalExactOutputReservationV1 {
                operation: Some(operation),
                pending: Some(pending),
                batch: Some(batch),
                authority: Some(retry_authority),
                wal_append,
            },
        ))
    }
    /// Consume one carrier-derived recovered Fetch through this exact service key.
    pub(in crate::sumeragi) fn authenticate_recovered_decision_fetch_request(
        &self,
        authority: RecoveredDecisionFetchRequestAuthorityV1,
    ) -> Result<RecoveredDecisionFetchRequestOwnerV1, String> {
        if self.io.is_none() || self.lifecycle_body_store_identity.is_none() {
            return Err(
                "recovered Decision Fetch requires the launched body-store worker".to_owned(),
            );
        }
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| {
                "recovered Decision Fetch request authentication requires restart".to_owned()
            })?;
        if !authority
            .identity
            .key()
            .matches_height_context(&self.context)
            || self.local_peer.public_key() != self.key_pair.public_key()
            || authority.round.context_id != self.context.id()
            || authority.round.height != self.context.height
            || authority.tag.height() != self.context.height
            || authority.sources
                != self
                    .context
                    .roster
                    .iter()
                    .map(|entry| entry.validator.clone())
                    .collect::<Vec<_>>()
        {
            return Err(
                "recovered Decision Fetch changed its fixed production service context".to_owned(),
            );
        }
        let mut request = wire::CertifiedBodyRequest {
            round: authority.round,
            subject: authority.subject,
            certificate: authority.certificate,
            requester: self.local_peer.clone(),
            signature: Vec::new(),
        };
        request.signature =
            Signature::try_new(self.key_pair.private_key(), &request.signature_preimage())
                .map_err(|error| error.to_string())?
                .payload()
                .to_vec();
        let authenticated = authenticate_certified_body_request_with_validator_pops(
            &self.context,
            &self.validator_set_pops,
            request,
            &self.local_peer,
        )
        .map_err(|error| error.to_string())?;
        let owner = RecoveredDecisionFetchRequestOwnerV1 {
            key: authority.identity.key(),
            tag: authority.tag,
            sources: authority.sources,
            authenticated,
            response_claim: None,
        };
        operation.complete();
        Ok(owner)
    }
    fn recovered_decision_fetch_fanout(
        &self,
        owner: &RecoveredDecisionFetchRequestOwnerV1,
    ) -> Result<Option<PendingExactFanout>, String> {
        if !owner.validates_exact_executor_context(&self.context, &self.local_peer)
            || self.exact_output_handoff_owner.is_sealed()
        {
            return Err(
                "recovered Decision Fetch output belongs to another service cut".to_owned(),
            );
        }
        let message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::CertifiedBodyRequest(
                owner.authenticated.request().clone(),
            ));
        let encoded = Self::preencode_v2_network_message(message)?;
        let peers = owner
            .sources
            .iter()
            .filter(|peer| *peer != &self.local_peer)
            .cloned()
            .collect::<Vec<_>>();
        PendingExactFanout::claimed(
            vec![encoded],
            peers,
            ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
        )
    }

    /// Freeze output/worker cuts for one preencoded recovered-Completion plan
    /// until typed selection or explicit no-selection completion.
    pub(in crate::sumeragi) fn capture_recovered_completion_capacity_census(
        &self,
        probes: Vec<RecoveredCompletionCapacityProbeV1>,
    ) -> Result<RecoveredCompletionCapacityCensusV1<'_>, String> {
        let io = self
            .io
            .as_ref()
            .ok_or_else(|| "recovered Completion census requires the launched worker".to_owned())?;
        if probes.is_empty() || self.exact_output_handoff_owner.is_sealed() {
            return Err("recovered Completion census has no live service cut".to_owned());
        }
        let mut candidates = BTreeMap::new();
        let mut apply_keys = BTreeSet::new();
        let mut sign_keys = BTreeSet::new();
        let mut fetch_keys = BTreeSet::new();
        for probe in probes {
            let (ordinal, candidate) = match probe {
                RecoveredCompletionCapacityProbeV1::Apply { ordinal, key } => {
                    if !key.matches_height_context(&self.context) || !apply_keys.insert(key) {
                        return Err(
                            "recovered Completion census changed an Apply dispatch key".to_owned()
                        );
                    }
                    (
                        ordinal,
                        RecoveredCompletionPreparedCapacityV1::Apply {
                            key,
                            available: false,
                        },
                    )
                }
                RecoveredCompletionCapacityProbeV1::Sign { ordinal, key } => {
                    if !key.matches_height_context(&self.context) || !sign_keys.insert(key) {
                        return Err(
                            "recovered Completion census changed a Sign dispatch key".to_owned()
                        );
                    }
                    (
                        ordinal,
                        RecoveredCompletionPreparedCapacityV1::Sign {
                            key,
                            available: false,
                        },
                    )
                }
                RecoveredCompletionCapacityProbeV1::Fetch {
                    ordinal,
                    owner,
                    executor_available,
                } => {
                    if !fetch_keys.insert(owner.dispatch_key()) {
                        return Err(
                            "recovered Completion census repeated a Fetch dispatch key".to_owned()
                        );
                    }
                    let fanout = self.recovered_decision_fetch_fanout(&owner)?;
                    (
                        ordinal,
                        RecoveredCompletionPreparedCapacityV1::Fetch {
                            owner,
                            fanout,
                            available: executor_available,
                        },
                    )
                }
            };
            if candidates.insert(ordinal, candidate).is_some() {
                return Err("recovered Completion census repeated one Ready ordinal".to_owned());
            }
        }
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "recovered Completion census requires restart".to_owned())?;
        let pending = self.lock_pending_exact_output()?;
        let state = io.command_tx.queue.lock();
        let mut census = RecoveredCompletionCapacityCensusV1 {
            operation: Some(operation),
            pending: Some(pending),
            queue: io.command_tx.queue.as_ref(),
            state: Some(state),
            worker_predecessor_debt: 0,
            output_predecessor_debt: 0,
            candidates,
        };
        if self.exact_output_handoff_owner.is_sealed()
            || census
                .state
                .as_ref()
                .is_none_or(|state| !state.sender_open || !state.receiver_open)
        {
            return Err("recovered Completion service cut closed during capture".to_owned());
        }
        census.worker_predecessor_debt = u64::try_from(
            census
                .state
                .as_ref()
                .expect("armed census retains its worker cut")
                .commands
                .len(),
        )
        .map_err(|_| "recovered Completion worker debt overflowed".to_owned())?;
        census.output_predecessor_debt = u64::try_from(
            census
                .pending
                .as_ref()
                .expect("armed census retains its output cut")
                .fanouts
                .len(),
        )
        .map_err(|_| "recovered Completion output debt overflowed".to_owned())?;
        let state = census
            .state
            .as_ref()
            .expect("armed census retains its worker cut");
        let pending = census
            .pending
            .as_ref()
            .expect("armed census retains its output cut");
        for candidate in census.candidates.values_mut() {
            match candidate {
                RecoveredCompletionPreparedCapacityV1::Apply { key, available } => {
                    if state.recovered_decision_applies.contains_key(key) {
                        return Err("recovered Completion Apply is already worker-owned".to_owned());
                    }
                    *available = io
                        .command_tx
                        .queue
                        .recovered_completion_worker_capacity(state);
                }
                RecoveredCompletionPreparedCapacityV1::Sign { key, available } => {
                    if state.recovered_lifecycle_signs.contains_key(key) {
                        return Err("recovered Completion Sign is already worker-owned".to_owned());
                    }
                    *available = io
                        .command_tx
                        .queue
                        .recovered_completion_worker_capacity(state);
                }
                RecoveredCompletionPreparedCapacityV1::Fetch {
                    fanout, available, ..
                } => {
                    *available = *available
                        && fanout
                            .as_ref()
                            .map_or(Ok(true), |fanout| pending.can_enqueue(fanout))?;
                }
            }
        }
        Ok(census)
    }

    /// Return whether this service and executor share one canonical output gate.
    pub(in crate::sumeragi) fn matches_lifecycle_executor_output_guard(
        &self,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
    ) -> bool {
        executor.matches_lifecycle_output_guard(&self.output_guard)
    }
    /// Return whether one lane adapter shares this exact height and storage owner.
    pub(in crate::sumeragi) fn matches_lifecycle_lane_work(
        &self,
        lane_work: &V2LaneWorkAdapter,
    ) -> bool {
        lane_work.matches_lifecycle_dependencies(
            &self.context,
            &self.state,
            &self.kura,
            &self.output_guard,
            &self.local_peer,
            &self.exact_output_handoff_owner,
        )
    }

    /// Authenticate the applied State and durable Kura tip for no-clock recovery.
    pub(in crate::sumeragi) fn matches_installed_pending_kura_tip(
        &self,
        expected: crate::sumeragi::v2_recovery::PendingKuraApply,
    ) -> bool {
        let Ok(height) = usize::try_from(expected.height()) else {
            return false;
        };
        let Some(height) = std::num::NonZeroUsize::new(height) else {
            return false;
        };
        self.context.id() == expected.context_id()
            && self.context.height == expected.height()
            && self.state.matches_kura_instance(&self.kura)
            && self.state.committed_height() == height.get()
            && self.state.latest_block_hash_fast() == Some(expected.block_hash())
            && self.kura.get_durable_block_hash(height) == Some(expected.block_hash())
    }

    fn owns_recovered_decision_apply_queue(&self, queue: &Arc<V2IoCommandQueue>) -> bool {
        self.io
            .as_ref()
            .is_some_and(|io| Arc::ptr_eq(&io.command_tx.queue, queue))
    }
    /// Return whether the live worker owns the exact body-store instance
    /// transferred by the lifecycle owner.
    pub(crate) fn matches_lifecycle_body_store(
        &self,
        owner_identity: &V2BodyStoreInstanceIdentity,
    ) -> bool {
        self.io.is_some()
            && self
                .lifecycle_body_store_identity
                .as_ref()
                .is_some_and(|worker_identity| worker_identity.same_instance(owner_identity))
    }

    /// Return whether the service was launched beside this exact Serve store.
    pub(in crate::sumeragi) fn matches_lifecycle_payload_store(
        &self,
        owner_identity: &CertifiedServePayloadStoreInstanceIdentity,
    ) -> bool {
        self.io.is_some()
            && self
                .lifecycle_payload_store_identity
                .as_ref()
                .is_some_and(|service_identity| service_identity.same_instance(owner_identity))
    }

    /// Refresh the live all-row Serve-retirement cut after the irreversible
    /// output seal, bound by launch permit and both store identities.
    pub(in crate::sumeragi) fn authenticate_current_lifecycle_serve_retirement(
        &self,
        permit: ProductionLifecycleServeRetirementAuthenticationPermitV1,
        verified: &super::v2::VerifiedHeightContext,
        payload_store: &CertifiedServePayloadStoreV1,
        owner_body_store_identity: &V2BodyStoreInstanceIdentity,
    ) -> Result<
        AuthenticatedCertifiedServePayloadRecoveryCut,
        CertifiedServeRetirementAuthenticationErrorV1,
    > {
        let payload_store_identity = payload_store.instance_identity();
        let roster_position = self
            .context
            .roster
            .iter()
            .position(|entry| entry.validator == self.local_peer)
            .and_then(|position| wire::ValidatorIndex::try_from(position).ok());
        if self.context != *verified.context()
            || self.validator_set_pops != verified.proofs_of_possession()
            || self.local_peer.public_key() != self.key_pair.public_key()
            || self
                .local_validator
                .is_some_and(|validator| roster_position != Some(validator))
            || !self.matches_lifecycle_body_store(owner_body_store_identity)
            || !self.matches_lifecycle_payload_store(&payload_store_identity)
            || !self.exact_output_handoff_owner.is_sealed()
        {
            return Err(CertifiedServeRetirementAuthenticationErrorV1::ForeignServiceOwner);
        }
        payload_store.authenticate_current_for_lifecycle_retirement(
            permit,
            verified,
            &self.key_pair,
        )
    }

    /// Seal an empty fixture corridor before exercising retirement census joins.
    #[cfg(test)]
    pub(in crate::sumeragi) fn seal_empty_exact_output_for_lifecycle_retirement_test(
        &self,
    ) -> Result<(), String> {
        let pending = self
            .pending_exact_output
            .lock()
            .map_err(|_| "fixture exact-output corridor lock was poisoned".to_owned())?;
        if pending.is_pending() {
            return Err("fixture exact-output corridor still owns output".to_owned());
        }
        self.exact_output_handoff_owner
            .seal()
            .map_err(|error| error.to_string())
    }

    fn recovered_lifecycle_next_vote_body_executor_permit<R: EffectRuntime>(
        &self,
        executor: &V2EffectExecutor<R>,
    ) -> Result<RecoveredLifecycleNextVoteBodyExecutorPermitV1, String> {
        let body_store_identity = self.lifecycle_body_store_identity.as_ref().ok_or_else(|| {
            "recovered next-Vote body authentication lost its launched store".to_owned()
        })?;
        if self.io.is_none()
            || !executor.matches_recovered_lifecycle_body_service(
                &self.context,
                &self.local_peer,
                &self.output_guard,
                body_store_identity,
            )
        {
            return Err(
                "recovered next-Vote body authentication found a foreign service owner".to_owned(),
            );
        }
        Ok(RecoveredLifecycleNextVoteBodyExecutorPermitV1::new(
            self.context.clone(),
            self.local_peer.clone(),
            Arc::clone(&self.output_guard),
            body_store_identity.clone(),
        ))
    }
    /// Preview recovered Sign and authenticate its successor in one joined
    /// worker/store borrow, avoiding a second executor preview.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion_with_body<'executor>(
        &self,
        executor: &'executor mut V2EffectExecutor<SerializedV2Runtime>,
        completion: RecoveredLifecycleSignAdapterCompletionAuthorityV1,
    ) -> Result<
        (
            super::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<'executor>,
            super::v2::RecoveredLifecycleNextVoteBodyAuthorityV1,
        ),
        String,
    > {
        let permit = self.recovered_lifecycle_next_vote_body_executor_permit(executor)?;
        executor
            .prepare_recovered_lifecycle_sign_completion_with_body(permit, completion)
            .map_err(|error| error.to_string())
    }
    /// Publish the completion owner only through the launch stack's move-only
    /// permit during final all-or-restart runner activation.
    #[allow(dead_code)]
    pub(in crate::sumeragi) fn activate_effect_completion_observer(
        &self,
        _permit: ProductionV2CompletionObserverActivationPermitV1,
    ) -> Result<(), String> {
        let activation_guard = Arc::clone(&self.output_guard);
        let activation = activation_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| {
                "Sumeragi v2 completion observer activation requires process restart".to_owned()
            })?;
        let io = self
            .io
            .as_ref()
            .ok_or_else(|| "Sumeragi v2 completion observer lost its live worker".to_owned())?;
        super::status::set_v2_effect_completion_observer(
            self.context.id(),
            self.context.height,
            &io.admission,
        );
        activation.complete();
        Ok(())
    }
    /// Reserve a selected lifecycle target after proving live worker/output,
    /// retaining queue, admission, and fail-stop operation in one borrow.
    #[allow(clippy::result_large_err)]
    pub(crate) fn capture_lifecycle_capacity_rank<'a>(
        &'a self,
        mut prepared: PreparedLifecycleIngressSelector,
    ) -> Result<LifecycleIoCapacityCapture<'a>, LifecycleIoCapacityCaptureError> {
        let Some(io) = self.io.as_ref() else {
            return Err(LifecycleIoCapacityCaptureError {
                failure: LifecycleIoCapacityCaptureFailure::Disconnected,
                prepared,
            });
        };
        let target = match prepared.take_lifecycle_io_target() {
            Ok(target) => target,
            Err(_) => {
                return Err(LifecycleIoCapacityCaptureError {
                    failure: LifecycleIoCapacityCaptureFailure::InvalidTarget,
                    prepared,
                });
            }
        };
        let target_context = target.context();
        if target_context.height() != self.context.height
            || target_context.id().as_bytes() != self.context.id().0.as_ref()
        {
            prepared
                .restore_lifecycle_io_target(target)
                .expect("the just-consumed selector target must restore exactly");
            return Err(LifecycleIoCapacityCaptureError {
                failure: LifecycleIoCapacityCaptureFailure::ForeignContext,
                prepared,
            });
        }
        let Some(operation) = self.output_guard.begin_fail_stop_operation() else {
            prepared
                .restore_lifecycle_io_target(target)
                .expect("the output-rejected selector target must restore exactly");
            return Err(LifecycleIoCapacityCaptureError {
                failure: LifecycleIoCapacityCaptureFailure::OutputClosed,
                prepared,
            });
        };
        match io.command_tx.queue.capture_lifecycle_capacity(
            operation,
            Arc::clone(&self.output_guard),
            target,
        ) {
            Ok(V2IoLifecycleCapacityCapture::Reserved(reservation)) => {
                Ok(LifecycleIoCapacityCapture {
                    outcome: LifecycleIoCapacityOutcome::Reserved {
                        reservation,
                        prepared,
                    },
                })
            }
            Ok(V2IoLifecycleCapacityCapture::Unavailable(wait)) => Ok(LifecycleIoCapacityCapture {
                outcome: LifecycleIoCapacityOutcome::Unavailable { wait, prepared },
            }),
            Err((failure, target)) => {
                prepared
                    .restore_lifecycle_io_target(target)
                    .expect("the rejected selector target must restore exactly");
                Err(LifecycleIoCapacityCaptureError { failure, prepared })
            }
        }
    }
    /// Reserve the Consensus lane for one exact lifecycle-owned recovered Sign.
    ///
    /// This happens before coordinator claim. The locked reservation accepts
    /// only a borrow-bound registry projection with the same class-sensitive
    /// key and releases all capacity automatically on every pre-commit error.
    pub(in crate::sumeragi) fn capture_recovered_lifecycle_sign_capacity<'a>(
        &'a self,
        key: RecoveredLifecycleSignDispatchKeyV1,
    ) -> Result<
        RecoveredLifecycleSignCapacityCaptureV1<'a>,
        RecoveredLifecycleSignCapacityCaptureErrorV1,
    > {
        if !key.matches_height_context(&self.context) {
            return Err(RecoveredLifecycleSignCapacityCaptureErrorV1::ForeignContext);
        }
        let io = self
            .io
            .as_ref()
            .ok_or(RecoveredLifecycleSignCapacityCaptureErrorV1::Disconnected)?;
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or(RecoveredLifecycleSignCapacityCaptureErrorV1::OutputClosed)?;
        io.command_tx
            .queue
            .capture_recovered_lifecycle_sign_capacity(operation, key)
    }
    /// Create the actor-global lifecycle source before leader-wire recovery.
    ///
    /// The retired worker-owned Serve journal is deliberately not a first-release
    /// production input. Launch immediately advances this source through the
    /// durable leader-wire high-watermarks before opening ingress.
    pub(crate) fn restore_lifecycle_ordinal_source(
        _context: &wire::HeightContext,
        _chunk_root: impl AsRef<Path>,
        _observer_source_capacity: usize,
        _observer_per_source_capacity: usize,
    ) -> Result<RuntimeLifecycleOrdinalSource, String> {
        // First-release production no longer restores the retired worker-owned
        // Serve scheduler. Leader-wire recovery advances this fresh shared
        // source past every durable actor-global producer before ingress opens.
        Ok(RuntimeLifecycleOrdinalSource::after_high_watermark(0))
    }
    /// Start the ordered I/O adapter for one immutable height context.
    #[allow(clippy::too_many_arguments, dead_code)]
    pub(crate) fn start(
        context: wire::HeightContext,
        initial_tag: EventTag,
        durable_decided_subject: Option<wire::BlockSubject>,
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
        provider_ingest_finalized_archive: Option<
            Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>,
        >,
        reputation_finalized_archive: Option<
            Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>,
        >,
        block_cadence: Duration,
        genesis_account: iroha_data_model::account::AccountId,
        events_sender: EventsSender,
        consensus_io_capacity: usize,
        auxiliary_io_capacity: usize,
        orphan_chunk_capacity: usize,
        output_guard: Arc<ConsensusOutputGuard>,
        leader_wire_ingress: Arc<FairV2Ingress>,
        kura_replica_advert_refresh: Arc<KuraReplicaAdvertRefreshOwner>,
        leader_wire_recovery_authority:
            super::serviced_candidate_store::LeaderWireRecoveryAuthority,
        exact_output_handoff_owner: DurableExactOutputServiceOwner,
    ) -> Result<Self, String> {
        let apply_service = V2ApplyService::new(
            Arc::clone(&state),
            queue,
            Arc::clone(&kura),
            provider_ingest_finalized_archive,
            reputation_finalized_archive,
            block_cadence,
            genesis_account,
            events_sender,
            validator_set_pops.clone(),
        );
        Self::start_inner(
            context,
            initial_tag,
            durable_decided_subject,
            validator_set_pops,
            local_peer,
            local_validator,
            key_pair,
            network,
            chunk_root,
            body_store,
            None,
            state,
            kura,
            apply_service,
            consensus_io_capacity,
            auxiliary_io_capacity,
            orphan_chunk_capacity,
            output_guard,
            leader_wire_ingress,
            kura_replica_advert_refresh,
            leader_wire_recovery_authority,
            exact_output_handoff_owner,
        )
    }
    /// Start with the replay application service, validating State, Kura,
    /// network identity, and roster before directories or workers exist.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn start_with_apply_service(
        _permit: super::v2_lifecycle_coordinator::ProductionLifecycleApplyServiceLaunchPermitV1,
        context: wire::HeightContext,
        initial_tag: EventTag,
        durable_decided_subject: Option<wire::BlockSubject>,
        validator_set_pops: Vec<Vec<u8>>,
        local_peer: PeerId,
        local_validator: Option<wire::ValidatorIndex>,
        key_pair: KeyPair,
        network: IrohaNetwork,
        chunk_root: impl AsRef<Path>,
        body_store: V2BodyStore,
        payload_store_identity: CertifiedServePayloadStoreInstanceIdentity,
        state: Arc<crate::state::State>,
        kura: Arc<crate::kura::Kura>,
        apply_service: V2ApplyService,
        consensus_io_capacity: usize,
        auxiliary_io_capacity: usize,
        orphan_chunk_capacity: usize,
        output_guard: Arc<ConsensusOutputGuard>,
        leader_wire_ingress: Arc<FairV2Ingress>,
        kura_replica_advert_refresh: Arc<KuraReplicaAdvertRefreshOwner>,
        leader_wire_recovery_authority:
            super::serviced_candidate_store::LeaderWireRecoveryAuthority,
        exact_output_handoff_owner: DurableExactOutputServiceOwner,
    ) -> Result<Self, String> {
        if !state.matches_kura_instance(&kura)
            || !apply_service.matches_lifecycle_launch(&state, &kura, &context, &validator_set_pops)
        {
            return Err(
                "Sumeragi v2 recovered Apply service changed lifecycle identity".to_owned(),
            );
        }
        Self::start_inner(
            context,
            initial_tag,
            durable_decided_subject,
            validator_set_pops,
            local_peer,
            local_validator,
            key_pair,
            network,
            chunk_root,
            body_store,
            Some(payload_store_identity),
            state,
            kura,
            apply_service,
            consensus_io_capacity,
            auxiliary_io_capacity,
            orphan_chunk_capacity,
            output_guard,
            leader_wire_ingress,
            kura_replica_advert_refresh,
            leader_wire_recovery_authority,
            exact_output_handoff_owner,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn start_inner(
        context: wire::HeightContext,
        initial_tag: EventTag,
        _durable_decided_subject: Option<wire::BlockSubject>,
        validator_set_pops: Vec<Vec<u8>>,
        local_peer: PeerId,
        local_validator: Option<wire::ValidatorIndex>,
        key_pair: KeyPair,
        network: IrohaNetwork,
        chunk_root: impl AsRef<Path>,
        body_store: V2BodyStore,
        lifecycle_payload_store_identity: Option<CertifiedServePayloadStoreInstanceIdentity>,
        state: Arc<crate::state::State>,
        kura: Arc<crate::kura::Kura>,
        apply_service: V2ApplyService,
        consensus_io_capacity: usize,
        auxiliary_io_capacity: usize,
        orphan_chunk_capacity: usize,
        output_guard: Arc<ConsensusOutputGuard>,
        leader_wire_ingress: Arc<FairV2Ingress>,
        kura_replica_advert_refresh: Arc<KuraReplicaAdvertRefreshOwner>,
        leader_wire_recovery_authority:
            super::serviced_candidate_store::LeaderWireRecoveryAuthority,
        exact_output_handoff_owner: DurableExactOutputServiceOwner,
    ) -> Result<Self, String> {
        let construction_guard = Arc::clone(&output_guard);
        let construction = construction_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if consensus_io_capacity == 0 || auxiliary_io_capacity == 0 || orphan_chunk_capacity == 0 {
            return Err("Sumeragi v2 service queue capacities must be non-zero".to_owned());
        }
        if initial_tag.height() != context.height {
            return Err(
                "Sumeragi v2 service tag is outside its immutable height context".to_owned(),
            );
        }
        let context_chunk_root = chunk_root
            .as_ref()
            .join(hex::encode(context.id().0.as_ref()));
        let max_orphan_chunk_bytes = maximum_orphan_chunk_bytes(context.da_layout);
        let max_messages_per_fanout = usize::try_from(context.da_layout.max_chunk_count)
            .map_err(|_| "Sumeragi v2 outbound chunk count is not representable".to_owned())?
            .checked_add(1)
            .ok_or_else(|| "Sumeragi v2 outbound fanout message bound overflowed".to_owned())?;
        let reply_route_source_capacity = network.reply_route_source_capacity().max(1);
        let max_peers_per_fanout = context.roster.len().max(reply_route_source_capacity).max(1);
        // Serve lifecycle storage has a frozen roster partition plus the
        // existing bounded authenticated reply-source partition. Each source
        // may own at most the already-configured auxiliary capacity; no new
        // environment or wire limit is introduced.
        // Capacity is charged per outstanding ordinary target/class occurrence.
        // Async producers and one reducer macro-step bound the shared unit pool;
        // frozen validator target/classes plus one topology-progress unit and one
        // fanout-level responder-control unit per frozen target are checked-added
        // separately. A responder control's exact authenticated routes remain
        // independently source-FIFO-indexed and bounded by the protocol fanout,
        // but cannot borrow shared capacity merely because one replay reached
        // several return paths. Only the configured authenticated-source count
        // can form an entirely non-frozen ordinary fanout, so require that
        // source-sized fanout to fit without charging the frozen roster twice.
        let shared_pending_ownership_unit_capacity =
            sumeragi_v2_exact_output_shared_ownership_capacity(
                consensus_io_capacity,
                auxiliary_io_capacity,
            )
            .map_err(|error| error.to_string())?;
        validate_shared_ownership_geometry(
            shared_pending_ownership_unit_capacity,
            reply_route_source_capacity,
        )?;
        let frozen_semantic_targets = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let pending_exact_output = PendingExactOutput::new(
            shared_pending_ownership_unit_capacity,
            max_messages_per_fanout,
            max_peers_per_fanout,
            &frozen_semantic_targets,
        )?;
        std::fs::create_dir_all(&context_chunk_root).map_err(|error| error.to_string())?;
        let durable_history = Arc::clone(&kura);
        let evidence_state = Arc::clone(&state);
        let certified_serve_validator_set_pops = validator_set_pops.clone();
        let lifecycle_body_store_identity = body_store.instance_identity();
        let io = V2IoHandle::spawn(
            body_store,
            apply_service,
            context.clone(),
            key_pair.clone(),
            local_validator,
            auxiliary_io_capacity,
            consensus_io_capacity,
            reply_route_source_capacity,
            Arc::clone(&output_guard),
        )?;
        let mut service = Self {
            context,
            validator_set_pops: certified_serve_validator_set_pops,
            state: evidence_state,
            local_peer,
            local_validator,
            key_pair,
            network,
            kura: durable_history,
            chunk_root: context_chunk_root,
            io: Some(io),
            lifecycle_body_store_identity: Some(lifecycle_body_store_identity),
            lifecycle_payload_store_identity,
            fetches: BTreeMap::new(),
            fetch_by_manifest: BTreeMap::new(),
            orphan_chunks: BTreeMap::new(),
            orphan_chunk_count: 0,
            orphan_chunk_bytes: 0,
            orphan_lifecycle_sweep_cursor: None,
            max_orphan_chunks: orphan_chunk_capacity,
            max_orphan_chunk_bytes,
            max_merge_sidecar_deferrals: consensus_io_capacity,
            local_completions: VecDeque::new(),
            held_io_completion: None,
            next_completion_source: CompletionSource::Io,
            locked_candidate_acquisition: None,
            next_locked_candidate_acquisition_id: 0,
            proposal_work_retired: false,
            prepared_candidates: VecDeque::new(),
            validation_rejections: VecDeque::new(),
            merge_sidecar_deferrals: VecDeque::new(),
            outbound_chunks: BTreeMap::new(),
            fast_path_proposals: BTreeSet::new(),
            pending_exact_output: Mutex::new(pending_exact_output),
            kura_replica_advert_refresh,
            exact_output_handoff_owner,
            #[cfg(test)]
            exact_output_admission_hook: None,
            active_tag: initial_tag,
            last_status: None,
            fatal_reason: None,
            output_guard,
            leader_wire_ingress,
            leader_wire_recovery_authority,
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
        owner: EventTag,
        payload: EncodedV2Payload,
    ) -> Result<wire::PayloadManifest, String> {
        if self.proposal_work_retired {
            return Err("Sumeragi v2 proposal work is terminal after Decision".to_owned());
        }
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
        let expected_round = wire::ConsensusRound {
            context_id: self.context.id(),
            height: self.context.height,
            view: owner.view(),
        };
        if owner != self.active_tag || manifest.round != expected_round {
            return Err(
                "Sumeragi v2 outbound payload is not owned by the active reducer incarnation"
                    .to_owned(),
            );
        }
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
        let retained = RetainedOutboundPayload {
            owner,
            round: manifest.round,
            subject: manifest.subject,
            messages,
        };
        if let Some(existing) = self.outbound_chunks.get(&manifest_hash) {
            if existing != &retained {
                return Err("conflicting local Sumeragi v2 payload manifest".to_owned());
            }
            self.outbound_chunks
                .retain(|hash, _| *hash == manifest_hash);
        } else {
            // There is one local proposal intent for an exact reducer owner.
            // A deterministic fallback or a higher same-tag lock supersedes
            // its old chunks before the replacement can enter signing.
            self.outbound_chunks.clear();
            self.outbound_chunks.insert(manifest_hash, retained);
        }
        operation.complete();
        Ok(manifest)
    }
    fn restore_outbound_payload_after_signature(
        &mut self,
        disposition: CompletionDisposition,
        payload: Option<EncodedV2Payload>,
    ) -> Result<(), String> {
        match disposition {
            CompletionDisposition::Accepted => {
                if let Some(payload) = payload {
                    self.register_outbound_payload(self.active_tag, payload)?;
                }
                Ok(())
            }
            CompletionDisposition::Stale => Ok(()),
            CompletionDisposition::Deferred | CompletionDisposition::Rejected => Err(
                "Sumeragi v2 signature completion returned a non-signature disposition".to_owned(),
            ),
        }
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
    fn plan_exact_body_fetch_owner_removal(
        &self,
        task: &BodyFetchTask,
    ) -> Result<BodyFetchServiceOwner, String> {
        let owner = self.body_fetch_service_owner(task.id())?;
        match owner {
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
            }
            BodyFetchServiceOwner::None => {
                return Err(format!(
                    "Sumeragi v2 body-fetch work {} has no service owner",
                    task.id().get()
                ));
            }
        }
        Ok(owner)
    }
    pub(in crate::sumeragi) fn prepare_certified_body_fetch_owner_removal(
        &mut self,
        task: &BodyFetchTask,
    ) -> Result<PreparedCertifiedBodyFetchOwnerRemoval<'_>, String> {
        if task.certified_request().is_none() {
            return Err(format!(
                "Sumeragi v2 body-fetch work {} completed without certified authority",
                task.id().get()
            ));
        }
        let owner = self.plan_exact_body_fetch_owner_removal(task)?;
        Ok(PreparedCertifiedBodyFetchOwnerRemoval {
            services: self,
            task: task.clone(),
            owner,
        })
    }
    /// Clone the process output guard before an exact service-removal token
    /// exclusively borrows this service owner.
    pub(in crate::sumeragi) fn lifecycle_output_guard(&self) -> Arc<ConsensusOutputGuard> {
        Arc::clone(&self.output_guard)
    }
    fn commit_exact_body_fetch_owner_removal(
        &mut self,
        task: &BodyFetchTask,
        owner: BodyFetchServiceOwner,
    ) {
        match owner {
            BodyFetchServiceOwner::Live => {
                self.fetches
                    .remove(&task.id())
                    .expect("preflighted live body-fetch owner remains present");
                if let Some(manifest_hash) = task.manifest().map(HashOf::new) {
                    let removed = self.fetch_by_manifest.remove(&manifest_hash);
                    debug_assert_eq!(removed, Some(task.id()));
                }
            }
            BodyFetchServiceOwner::Reconstructed(index) => {
                self.local_completions
                    .remove(index)
                    .expect("preflighted queued body-fetch owner remains present");
            }
            BodyFetchServiceOwner::None => {
                unreachable!("exact body-fetch removal preflight excludes an absent owner")
            }
        }
    }
    fn remove_exact_body_fetch_owner(&mut self, task: &BodyFetchTask) -> Result<(), String> {
        let owner = self.plan_exact_body_fetch_owner_removal(task)?;
        self.commit_exact_body_fetch_owner_removal(task, owner);
        Ok(())
    }
    /// Load a lock-constrained body by immutable subject so view rebinding adds
    /// no same-subject disk read.
    pub(crate) fn request_locked_candidate(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<(), String> {
        if self.proposal_work_retired {
            return Ok(());
        }
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if tag.height() != self.context.height
            || round.context_id != self.context.id()
            || round.height != self.context.height
            || round.view > tag.view()
        {
            return Err(
                "Sumeragi v2 locked-body request has an invalid round/tag context".to_owned(),
            );
        }
        if self.locked_candidate_acquisition.is_some() {
            let rebound = self
                .locked_candidate_acquisition
                .as_mut()
                .expect("acquisition presence checked above")
                .rebind_consumer(round, subject, tag)?;
            if matches!(
                rebound,
                LockedCandidateRebind::ConsumerAdvanced
                    | LockedCandidateRebind::ReplacementDeferred
                    | LockedCandidateRebind::ReplacementRequired
            ) {
                iroha_logger::debug!(
                    height = tag.height(),
                    view = tag.view(),
                    generation = tag.generation().get(),
                    ?subject,
                    "rebound exact locked-body acquisition to current Sumeragi v2 view"
                );
            }
            if rebound == LockedCandidateRebind::ReplacementRequired {
                let acquisition_id = self.allocate_locked_candidate_acquisition_id()?;
                self.enqueue_locked_candidate_load(acquisition_id, subject)?;
                self.locked_candidate_acquisition
                    .as_mut()
                    .expect("ready acquisition remains owned during replacement")
                    .start_replacement(acquisition_id);
            }
            operation.complete();
            return Ok(());
        }
        let acquisition_id = self.allocate_locked_candidate_acquisition_id()?;
        self.enqueue_locked_candidate_load(acquisition_id, subject)?;
        self.locked_candidate_acquisition = Some(LockedCandidateAcquisition::loading(
            acquisition_id,
            round,
            subject,
            tag,
        ));
        iroha_logger::debug!(
            height = tag.height(),
            view = tag.view(),
            generation = tag.generation().get(),
            ?subject,
            "queued exact locked-body load for Sumeragi v2 re-proposal"
        );
        operation.complete();
        Ok(())
    }
    /// Borrow the immutable height-local signer only for lifecycle-owned
    /// Certified-Serve payload admission.
    pub(in crate::sumeragi) const fn lifecycle_local_signer(&self) -> &KeyPair {
        &self.key_pair
    }
    /// Return whether this exact worker can serve the authenticated request.
    ///
    /// Non-retainers are rejected as transport traffic before lifecycle
    /// publication; they must never create a Ready row which the worker can
    /// only fail after dequeue.
    pub(in crate::sumeragi) fn lifecycle_certified_serve_is_locally_authorized(
        &self,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> bool {
        self.local_validator.is_some_and(|validator| {
            authenticated
                .request()
                .certificate
                .signers
                .binary_search(&validator)
                .is_ok()
        })
    }
    /// Reserve the auxiliary worker class for an already prelocked Serve target.
    pub(in crate::sumeragi) fn capture_lifecycle_certified_serve_capacity<'a>(
        &'a self,
        target: LifecycleIngressIoTargetSeal,
    ) -> Result<LifecycleCertifiedServeCapacityCaptureV1<'a>, LifecycleIoCapacityCaptureFailure>
    {
        let Some(io) = self.io.as_ref() else {
            return Err(LifecycleIoCapacityCaptureFailure::Disconnected);
        };
        let target_context = target.context();
        if target.kind() != LifecycleIngressIoTargetKind::CertifiedServe {
            return Err(LifecycleIoCapacityCaptureFailure::InvalidTarget);
        }
        if target_context.height() != self.context.height
            || target_context.id().as_bytes() != self.context.id().0.as_ref()
        {
            return Err(LifecycleIoCapacityCaptureFailure::ForeignContext);
        }
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or(LifecycleIoCapacityCaptureFailure::OutputClosed)?;
        match io.command_tx.queue.capture_lifecycle_capacity(
            operation,
            Arc::clone(&self.output_guard),
            target,
        ) {
            Ok(V2IoLifecycleCapacityCapture::Reserved(reservation)) => Ok(
                LifecycleCertifiedServeCapacityCaptureV1::Reserved(reservation),
            ),
            Ok(V2IoLifecycleCapacityCapture::Unavailable(wait)) => {
                Ok(LifecycleCertifiedServeCapacityCaptureV1::Unavailable(
                    LifecycleCertifiedServeCapacityWaitV1 { wait },
                ))
            }
            Err((failure, _target)) => Err(failure),
        }
    }
    fn allocate_locked_candidate_acquisition_id(
        &mut self,
    ) -> Result<LockedCandidateAcquisitionId, String> {
        let acquisition_id =
            LockedCandidateAcquisitionId(self.next_locked_candidate_acquisition_id);
        self.next_locked_candidate_acquisition_id = self
            .next_locked_candidate_acquisition_id
            .checked_add(1)
            .ok_or_else(|| "Sumeragi v2 locked-body acquisition ID overflow".to_owned())?;
        Ok(acquisition_id)
    }
    fn enqueue_locked_candidate_load(
        &self,
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
    ) -> Result<(), String> {
        self.io()?.enqueue(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject,
        })
    }
    fn complete_locked_candidate_load(
        &mut self,
        loaded: LockedCandidateLoad,
    ) -> Result<Option<EventTag>, String> {
        let completion = self
            .locked_candidate_acquisition
            .as_mut()
            .ok_or_else(|| {
                "Sumeragi v2 locked-body completion has no acquisition owner".to_owned()
            })?
            .complete(loaded)?;
        self.finish_locked_candidate_completion(completion)
    }
    fn locked_candidate_load_unavailable(
        &mut self,
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
    ) -> Result<Option<EventTag>, String> {
        let completion = self
            .locked_candidate_acquisition
            .as_mut()
            .ok_or_else(|| {
                "Sumeragi v2 locked-body unavailability has no acquisition owner".to_owned()
            })?
            .unavailable(acquisition_id, subject)?;
        self.finish_locked_candidate_completion(completion)
    }
    fn locked_candidate_load_failed(
        &mut self,
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
        reason: String,
    ) -> Result<Option<EventTag>, String> {
        let completion = self
            .locked_candidate_acquisition
            .as_ref()
            .ok_or_else(|| "Sumeragi v2 locked-body failure has no acquisition owner".to_owned())?
            .failed(acquisition_id, subject)
            .map_err(|classification| format!("{classification}: {reason}"))?;
        self.finish_locked_candidate_completion(completion)
    }
    fn finish_locked_candidate_completion(
        &mut self,
        completion: LockedCandidateCompletion,
    ) -> Result<Option<EventTag>, String> {
        match completion {
            LockedCandidateCompletion::Ready(tag) => Ok(Some(tag)),
            LockedCandidateCompletion::Stale | LockedCandidateCompletion::Waiting => Ok(None),
            LockedCandidateCompletion::ReplacementRequired => {
                let subject = self
                    .locked_candidate_acquisition
                    .as_ref()
                    .expect("superseded acquisition remains owned during replacement")
                    .subject;
                let acquisition_id = self.allocate_locked_candidate_acquisition_id()?;
                self.enqueue_locked_candidate_load(acquisition_id, subject)?;
                self.locked_candidate_acquisition
                    .as_mut()
                    .expect("superseded acquisition remains owned during replacement")
                    .start_replacement(acquisition_id);
                Ok(None)
            }
        }
    }
    fn retry_locked_candidate_after_store(
        &mut self,
        subject: wire::BlockSubject,
    ) -> Result<(), String> {
        let should_retry = self
            .locked_candidate_acquisition
            .as_ref()
            .is_some_and(|acquisition| {
                acquisition.subject == subject
                    && matches!(
                        &acquisition.state,
                        LockedCandidateAcquisitionState::Waiting { .. }
                    )
            });
        if !should_retry {
            return Ok(());
        }
        let acquisition_id = self.allocate_locked_candidate_acquisition_id()?;
        self.enqueue_locked_candidate_load(acquisition_id, subject)?;
        self.locked_candidate_acquisition
            .as_mut()
            .expect("waiting acquisition remains owned during durable retry")
            .start_replacement(acquisition_id);
        Ok(())
    }
    /// Take the next locked-subject body loaded by the ordered I/O worker.
    pub(crate) fn take_loaded_candidate(&mut self) -> Option<LoadedCandidateBody> {
        if self.output_guard.restart_required() {
            return None;
        }
        self.locked_candidate_acquisition
            .as_mut()
            .and_then(LockedCandidateAcquisition::take_ready)
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
            // Capacity backpressure leaves the retained FIFO unchanged and
            // creates no ambiguous output at this service boundary.
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
    pub(crate) fn route_payload_chunk<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
        sender: PeerId,
        chunk: wire::PayloadChunk,
        ingress_ownership: FairV2IngressOwnershipEvidence,
    ) -> Result<PayloadChunkDisposition, String> {
        let chunk_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::PayloadChunk(chunk.clone()),
        ));
        if !ingress_ownership.validate_exact()
            || !ingress_ownership.matches_message(&chunk_message)
            || !ingress_ownership.matches_semantic_origin(Some(&sender))
        {
            return Err("payload chunk carried altered fair-ingress ownership".to_owned());
        }
        let manifest_hash = chunk.manifest_hash;
        if let Some(work_id) = self.fetch_work_for_manifest(manifest_hash) {
            return self.deliver_payload_chunk(executor, work_id, sender, chunk, ingress_ownership);
        }
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if let Some(runtime) = ingress_ownership.leader_wire_runtime_receipt() {
            if self.has_exact_reconstructed_completion(manifest_hash, &ingress_ownership)? {
                self.leader_wire_ingress
                    .mark_leader_wire_volatile_terminal(runtime)?;
                return Ok(PayloadChunkDisposition::Duplicate);
            }
            match executor
                .classify_payload_chunk_lifecycle(manifest_hash, &ingress_ownership)
                .map_err(|error| error.to_string())?
            {
                PayloadChunkLifecycleDisposition::Durable(receipt) => {
                    self.leader_wire_ingress
                        .mark_leader_wire_durable_body_terminal(runtime, &receipt)?;
                    return Ok(PayloadChunkDisposition::Duplicate);
                }
                PayloadChunkLifecycleDisposition::Volatile => {
                    self.leader_wire_ingress
                        .mark_leader_wire_volatile_terminal(runtime)?;
                    return Ok(PayloadChunkDisposition::Duplicate);
                }
                PayloadChunkLifecycleDisposition::Retain => {}
            }
        }
        let terminal_ownership = ingress_ownership.clone();
        match self.buffer_orphan_payload_chunk_owned_checked(sender, chunk, ingress_ownership) {
            OrphanPayloadChunkBufferResult::Disposition(disposition) => {
                if disposition == PayloadChunkDisposition::Rejected
                    && let Some(runtime) = terminal_ownership.leader_wire_runtime_receipt()
                {
                    self.leader_wire_ingress
                        .mark_leader_wire_volatile_terminal(runtime)?;
                }
                Ok(disposition)
            }
            OrphanPayloadChunkBufferResult::ProductiveRetentionConflict => {
                Err("bounded orphan storage could not retain an exact leader-wire owner".to_owned())
            }
        }
    }
    fn has_exact_reconstructed_completion(
        &self,
        manifest_hash: HashOf<wire::PayloadManifest>,
        ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> Result<bool, String> {
        let runtime = ingress_ownership
            .leader_wire_runtime_receipt()
            .ok_or_else(|| {
                "productive payload chunk lost its leader-wire runtime receipt".to_owned()
            })?;
        let token = runtime.token();
        if !token.matches_chunk_manifest(manifest_hash) {
            return Err(
                "reconstructed payload completion changed its leader-wire manifest".to_owned(),
            );
        }
        for completion in &self.local_completions {
            let LocalCompletion::Reconstructed { task, manifest, .. } = completion;
            if token.matches_exact_body(manifest.round, manifest.subject, HashOf::new(manifest)) {
                if !task.matches_reconstructed_manifest(manifest) {
                    return Err(
                        "queued payload reconstruction differs from its exact task".to_owned()
                    );
                }
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn buffer_orphan_payload_chunk_owned_checked(
        &mut self,
        sender: PeerId,
        chunk: wire::PayloadChunk,
        ingress_ownership: FairV2IngressOwnershipEvidence,
    ) -> OrphanPayloadChunkBufferResult {
        self.buffer_orphan_payload_chunk_inner(sender, chunk, Some(ingress_ownership))
    }
    #[cfg(test)]
    fn buffer_orphan_payload_chunk_owned(
        &mut self,
        sender: PeerId,
        chunk: wire::PayloadChunk,
        ingress_ownership: FairV2IngressOwnershipEvidence,
    ) -> PayloadChunkDisposition {
        self.buffer_orphan_payload_chunk_owned_checked(sender, chunk, ingress_ownership)
            .public_disposition()
    }
    #[cfg(test)]
    fn buffer_orphan_payload_chunk(
        &mut self,
        sender: PeerId,
        chunk: wire::PayloadChunk,
    ) -> PayloadChunkDisposition {
        self.buffer_orphan_payload_chunk_inner(sender, chunk, None)
            .public_disposition()
    }
    fn buffer_orphan_payload_chunk_inner(
        &mut self,
        sender: PeerId,
        chunk: wire::PayloadChunk,
        ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
    ) -> OrphanPayloadChunkBufferResult {
        let manifest_hash = chunk.manifest_hash;
        let productive_owner = ingress_ownership
            .as_ref()
            .is_some_and(|ownership| ownership.leader_wire_runtime_receipt().is_some());
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
            return OrphanPayloadChunkBufferResult::Disposition(PayloadChunkDisposition::Rejected);
        }
        let mut replaced_proofless = None;
        if let Some(buffered) = self.orphan_chunks.get_mut(&manifest_hash) {
            if let Some(existing) = buffered.iter_mut().find(|existing| {
                existing.sender == sender
                    && existing.chunk.index == chunk.index
                    && existing.chunk == chunk
            }) {
                let incumbent_productive = existing
                    .ingress_ownership
                    .as_ref()
                    .is_some_and(|ownership| ownership.leader_wire_runtime_receipt().is_some());
                if productive_owner && !incumbent_productive {
                    let Some(candidate) = ingress_ownership else {
                        return OrphanPayloadChunkBufferResult::ProductiveRetentionConflict;
                    };
                    // Proposal processing has now bound the same physical
                    // bytes to their immutable leader-wire lifecycle. Promote
                    // that exact carrier in place: count/byte geometry stays
                    // unchanged, and proofless eviction can no longer discard
                    // the canonical runtime owner.
                    existing.ingress_ownership = Some(candidate);
                    return OrphanPayloadChunkBufferResult::Disposition(
                        PayloadChunkDisposition::Duplicate,
                    );
                }
                match (&mut existing.ingress_ownership, ingress_ownership) {
                    (Some(retained), Some(candidate)) => {
                        if !retained.merge_downstream(candidate) {
                            return OrphanPayloadChunkBufferResult::Disposition(
                                PayloadChunkDisposition::Rejected,
                            );
                        }
                    }
                    (None, None) if cfg!(test) => {}
                    (Some(_), None) | (None, Some(_)) | (None, None) => {
                        return OrphanPayloadChunkBufferResult::Disposition(
                            PayloadChunkDisposition::Rejected,
                        );
                    }
                }
                return OrphanPayloadChunkBufferResult::Disposition(
                    PayloadChunkDisposition::Duplicate,
                );
            }
            // Retain at most one claim per authenticated outer sender/index. A
            // productive, manifest-bound owner replaces a proofless reordered
            // claim in the same slot. Otherwise the conflict cannot be
            // resolved until an existing productive owner retires.
            if let Some(position) = buffered.iter().position(|existing| {
                existing.sender == sender && existing.chunk.index == chunk.index
            }) {
                let incumbent_productive = buffered[position]
                    .ingress_ownership
                    .as_ref()
                    .is_some_and(|ownership| ownership.leader_wire_runtime_receipt().is_some());
                if productive_owner && !incumbent_productive {
                    replaced_proofless = buffered.remove(position);
                } else {
                    return if productive_owner {
                        OrphanPayloadChunkBufferResult::ProductiveRetentionConflict
                    } else {
                        OrphanPayloadChunkBufferResult::Disposition(
                            PayloadChunkDisposition::Rejected,
                        )
                    };
                }
            }
        }
        if let Some(replaced) = replaced_proofless {
            let replaced_bytes = u64::try_from(replaced.chunk.bytes.len()).unwrap_or(u64::MAX);
            self.orphan_chunk_count = self.orphan_chunk_count.saturating_sub(1);
            self.orphan_chunk_bytes = self.orphan_chunk_bytes.saturating_sub(replaced_bytes);
            if self
                .orphan_chunks
                .get(&manifest_hash)
                .is_some_and(VecDeque::is_empty)
            {
                self.orphan_chunks.remove(&manifest_hash);
            }
        }
        while productive_owner
            && (self.orphan_chunk_count >= self.max_orphan_chunks
                || self.orphan_chunk_bytes.saturating_add(chunk_len) > self.max_orphan_chunk_bytes)
        {
            if !self.evict_one_proofless_orphan_chunk() {
                return OrphanPayloadChunkBufferResult::ProductiveRetentionConflict;
            }
        }
        if self.orphan_chunk_count >= self.max_orphan_chunks
            || self.orphan_chunk_bytes.saturating_add(chunk_len) > self.max_orphan_chunk_bytes
        {
            return OrphanPayloadChunkBufferResult::Disposition(PayloadChunkDisposition::Rejected);
        }
        let buffered = self.orphan_chunks.entry(manifest_hash).or_default();
        buffered.push_back(BufferedPayloadChunk {
            sender,
            chunk,
            ingress_ownership,
        });
        self.orphan_chunk_count = self.orphan_chunk_count.saturating_add(1);
        self.orphan_chunk_bytes = self.orphan_chunk_bytes.saturating_add(chunk_len);
        OrphanPayloadChunkBufferResult::Disposition(PayloadChunkDisposition::Buffered)
    }
    fn evict_one_proofless_orphan_chunk(&mut self) -> bool {
        let selected = self
            .orphan_chunks
            .iter()
            .find_map(|(manifest_hash, chunks)| {
                chunks
                    .iter()
                    .position(|buffered| {
                        buffered.ingress_ownership.as_ref().is_none_or(|ownership| {
                            ownership.leader_wire_runtime_receipt().is_none()
                        })
                    })
                    .map(|position| (*manifest_hash, position))
            });
        let Some((manifest_hash, position)) = selected else {
            return false;
        };
        let (removed, remove_manifest) = {
            let chunks = self
                .orphan_chunks
                .get_mut(&manifest_hash)
                .expect("selected orphan manifest remains present");
            let removed = chunks
                .remove(position)
                .expect("selected proofless orphan remains present");
            (removed, chunks.is_empty())
        };
        if remove_manifest {
            self.orphan_chunks.remove(&manifest_hash);
        }
        let removed_bytes = u64::try_from(removed.chunk.bytes.len()).unwrap_or(u64::MAX);
        self.orphan_chunk_count = self.orphan_chunk_count.saturating_sub(1);
        self.orphan_chunk_bytes = self.orphan_chunk_bytes.saturating_sub(removed_bytes);
        true
    }
    fn next_orphan_payload_lifecycle_sweep_position(
        &self,
    ) -> Option<OrphanPayloadLifecycleSweepCursor> {
        let first = || {
            self.orphan_chunks
                .iter()
                .find(|(_, chunks)| !chunks.is_empty())
                .map(|(manifest_hash, _)| OrphanPayloadLifecycleSweepCursor {
                    manifest_hash: *manifest_hash,
                    chunk_offset: 0,
                })
        };
        let Some(cursor) = self.orphan_lifecycle_sweep_cursor else {
            return first();
        };
        if self
            .orphan_chunks
            .get(&cursor.manifest_hash)
            .is_some_and(|chunks| cursor.chunk_offset < chunks.len())
        {
            return Some(cursor);
        }
        self.orphan_chunks
            .range((
                std::ops::Bound::Excluded(cursor.manifest_hash),
                std::ops::Bound::Unbounded,
            ))
            .find(|(_, chunks)| !chunks.is_empty())
            .map(|(manifest_hash, _)| OrphanPayloadLifecycleSweepCursor {
                manifest_hash: *manifest_hash,
                chunk_offset: 0,
            })
            .or_else(first)
    }
    fn terminalize_buffered_payload_chunk_if_complete<R: EffectRuntime>(
        &self,
        executor: &V2EffectExecutor<R>,
        manifest_hash: HashOf<wire::PayloadManifest>,
        buffered: &BufferedPayloadChunk,
    ) -> Result<bool, String> {
        let Some(ingress_ownership) = buffered.ingress_ownership.as_ref() else {
            return Ok(false);
        };
        let Some(runtime) = ingress_ownership.leader_wire_runtime_receipt() else {
            return Ok(false);
        };
        let disposition =
            match self.has_exact_reconstructed_completion(manifest_hash, ingress_ownership) {
                Ok(true) => PayloadChunkLifecycleDisposition::Volatile,
                Ok(false) => executor
                    .classify_payload_chunk_lifecycle(manifest_hash, ingress_ownership)
                    .map_err(|error| error.to_string())?,
                Err(error) => return Err(error),
            };
        match disposition {
            PayloadChunkLifecycleDisposition::Durable(receipt) => self
                .leader_wire_ingress
                .mark_leader_wire_durable_body_terminal(runtime, &receipt)?,
            PayloadChunkLifecycleDisposition::Volatile => self
                .leader_wire_ingress
                .mark_leader_wire_volatile_terminal(runtime)?,
            PayloadChunkLifecycleDisposition::Retain => return Ok(false),
        }
        Ok(true)
    }
    fn sweep_buffered_payload_chunk_lifecycles<R: EffectRuntime>(
        &mut self,
        executor: &V2EffectExecutor<R>,
    ) -> Result<usize, String> {
        let mut retired = 0usize;
        let mut first_error = None;
        let visits = self
            .orphan_chunk_count
            .min(MAX_ORPHAN_LIFECYCLE_VISITS_PER_REPLAY);
        for _ in 0..visits {
            let Some(cursor) = self.next_orphan_payload_lifecycle_sweep_position() else {
                self.orphan_lifecycle_sweep_cursor = None;
                break;
            };
            let classification = {
                let buffered = self
                    .orphan_chunks
                    .get(&cursor.manifest_hash)
                    .and_then(|chunks| chunks.get(cursor.chunk_offset))
                    .expect("orphan lifecycle cursor resolves an existing buffered chunk");
                self.terminalize_buffered_payload_chunk_if_complete(
                    executor,
                    cursor.manifest_hash,
                    buffered,
                )
            };
            match classification {
                Ok(true) => {
                    let (removed, remove_manifest) = {
                        let chunks = self
                            .orphan_chunks
                            .get_mut(&cursor.manifest_hash)
                            .expect("classified orphan manifest remains present");
                        let removed = chunks
                            .remove(cursor.chunk_offset)
                            .expect("classified orphan chunk remains present");
                        (removed, chunks.is_empty())
                    };
                    if remove_manifest {
                        self.orphan_chunks.remove(&cursor.manifest_hash);
                    }
                    let bytes = u64::try_from(removed.chunk.bytes.len()).unwrap_or(u64::MAX);
                    self.orphan_chunk_count = self.orphan_chunk_count.saturating_sub(1);
                    self.orphan_chunk_bytes = self.orphan_chunk_bytes.saturating_sub(bytes);
                    retired = retired.saturating_add(1);
                    self.orphan_lifecycle_sweep_cursor = Some(cursor);
                }
                Ok(false) => {
                    self.orphan_lifecycle_sweep_cursor = Some(OrphanPayloadLifecycleSweepCursor {
                        manifest_hash: cursor.manifest_hash,
                        chunk_offset: cursor.chunk_offset.saturating_add(1),
                    });
                }
                Err(error) => {
                    self.orphan_lifecycle_sweep_cursor = Some(OrphanPayloadLifecycleSweepCursor {
                        manifest_hash: cursor.manifest_hash,
                        chunk_offset: cursor.chunk_offset.saturating_add(1),
                    });
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        first_error.map_or(Ok(retired), Err)
    }
    /// Replay all chunks whose proposal manifests have now opened sessions.
    pub(crate) fn replay_buffered_chunks<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
    ) -> Result<usize, String> {
        if self.output_guard.restart_required() {
            return Err("Sumeragi v2 consensus requires process restart".to_owned());
        }
        self.sweep_buffered_payload_chunk_lifecycles(executor)?;
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
                    if let Some(runtime) = buffered
                        .ingress_ownership
                        .as_ref()
                        .and_then(FairV2IngressOwnershipEvidence::leader_wire_runtime_receipt)
                        && let Err(error) = self
                            .leader_wire_ingress
                            .mark_leader_wire_volatile_terminal(runtime)
                    {
                        if let Err(tail_error) = self.retire_buffered_payload_chunk_tail(chunks) {
                            return Err(format!(
                                "{error}; additionally failed to retire buffered payload tail: {tail_error}"
                            ));
                        }
                        return Err(error);
                    }
                    continue;
                }
                let Some(ingress_ownership) = buffered.ingress_ownership else {
                    let tail_result = self.retire_buffered_payload_chunk_tail(chunks);
                    return Err(tail_result.err().unwrap_or_else(|| {
                        "buffered payload chunk lost fair-ingress ownership".to_owned()
                    }));
                };
                match self.deliver_payload_chunk(
                    executor,
                    work_id,
                    buffered.sender,
                    buffered.chunk,
                    ingress_ownership,
                ) {
                    Ok(PayloadChunkDisposition::Delivered) => {
                        delivered = delivered.saturating_add(1);
                    }
                    Ok(_) => {}
                    Err(error) => {
                        if let Err(tail_error) = self.retire_buffered_payload_chunk_tail(chunks) {
                            return Err(format!(
                                "{error}; additionally failed to retire buffered payload tail: {tail_error}"
                            ));
                        }
                        return Err(error);
                    }
                }
            }
        }
        Ok(delivered)
    }
    fn retire_buffered_payload_chunk_tail(
        &mut self,
        mut chunks: VecDeque<BufferedPayloadChunk>,
    ) -> Result<(), String> {
        let mut first_error = None;
        while let Some(buffered) = chunks.pop_front() {
            let bytes = u64::try_from(buffered.chunk.bytes.len()).unwrap_or(u64::MAX);
            self.orphan_chunk_count = self.orphan_chunk_count.saturating_sub(1);
            self.orphan_chunk_bytes = self.orphan_chunk_bytes.saturating_sub(bytes);
            let Some(runtime) = buffered
                .ingress_ownership
                .as_ref()
                .and_then(FairV2IngressOwnershipEvidence::leader_wire_runtime_receipt)
            else {
                continue;
            };
            if let Err(error) = self
                .leader_wire_ingress
                .mark_leader_wire_volatile_terminal(runtime)
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
    fn take_io_completion(&mut self, runtime_capacity_available: bool) -> IoCompletionTake {
        if self.held_io_completion.as_ref().is_some_and(|completion| {
            matches!(
                completion,
                V2IoCompletion::RecoveredDecisionApply(_)
                    | V2IoCompletion::RecoveredLifecycleSign(_)
                    | V2IoCompletion::RecoveredDecisionFetchBodyPersisted(_)
                    | V2IoCompletion::LifecycleCertifiedServe(_)
            )
        }) {
            return IoCompletionTake::retained_runtime();
        }
        if runtime_capacity_available && let Some(completion) = self.held_io_completion.take() {
            return IoCompletionTake::ready(PendingServiceCompletion::Io {
                completion,
                ownership_position: 0,
            });
        }
        let ownership_position =
            usize::from(!runtime_capacity_available && self.held_io_completion.is_some());
        let Some(io) = self.io.as_ref() else {
            return IoCompletionTake::unavailable();
        };
        if ownership_position != 0
            && io
                .completion_ownership_at(ownership_position)
                .is_some_and(|owned| {
                    owned.recovered_decision_apply.is_some()
                        || owned.recovered_lifecycle_sign.is_some()
                        || owned.recovered_decision_fetch.is_some()
                        || owned.lifecycle_certified_serve.is_some()
                })
        {
            // There is only one payload parking slot. A lifecycle-owned
            // completion behind an already-held runtime result must remain in
            // the physical channel until that result is serviced; receiving it
            // here would detach the payload from its keyed owner or overwrite
            // the held result.
            return IoCompletionTake::retained_runtime();
        }
        // Once the oldest runtime-producing result has crossed the physical
        // channel boundary, keep exactly that one result unacknowledged. The
        // ownership tracker lets us look past it only when the next published
        // result is known not to require a reducer-completion slot.
        if !runtime_capacity_available
            && ownership_position != 0
            && io.completion_requires_runtime_capacity_at(ownership_position) != Some(false)
        {
            return IoCompletionTake::unavailable();
        }
        let Ok(completion) = io.try_recv_completion_unacknowledged() else {
            return IoCompletionTake::unavailable();
        };
        if matches!(
            &completion,
            V2IoCompletion::RecoveredDecisionApply(_)
                | V2IoCompletion::RecoveredLifecycleSign(_)
                | V2IoCompletion::RecoveredDecisionFetchBodyPersisted(_)
                | V2IoCompletion::LifecycleCertifiedServe(_)
        ) {
            assert!(
                self.held_io_completion.is_none(),
                "completion ownership metadata must preserve one recovered lifecycle head"
            );
            self.held_io_completion = Some(completion);
            return IoCompletionTake::retained_runtime();
        }
        if !runtime_capacity_available && completion.requires_runtime_capacity() {
            assert!(
                self.held_io_completion.is_none(),
                "completion ownership metadata must prevent a second held runtime result"
            );
            self.held_io_completion = Some(completion);
            return IoCompletionTake::retained_runtime();
        }
        IoCompletionTake::ready(PendingServiceCompletion::Io {
            completion,
            ownership_position,
        })
    }
    fn take_recovered_lifecycle_sign_completion(&mut self) -> IoCompletionTake {
        if let Some(completion) = self.held_io_completion.take() {
            if matches!(&completion, V2IoCompletion::RecoveredLifecycleSign(_)) {
                return IoCompletionTake::ready(PendingServiceCompletion::Io {
                    completion,
                    ownership_position: 0,
                });
            }
            self.held_io_completion = Some(completion);
            return IoCompletionTake::unavailable();
        }
        let Some(io) = self.io.as_ref() else {
            return IoCompletionTake::unavailable();
        };
        let Ok(completion) = io.try_recv_completion_unacknowledged() else {
            return IoCompletionTake::unavailable();
        };
        if matches!(&completion, V2IoCompletion::RecoveredLifecycleSign(_)) {
            IoCompletionTake::ready(PendingServiceCompletion::Io {
                completion,
                ownership_position: 0,
            })
        } else {
            self.held_io_completion = Some(completion);
            IoCompletionTake::unavailable()
        }
    }
    fn take_lifecycle_certified_serve_completion(&mut self) -> IoCompletionTake {
        if let Some(completion) = self.held_io_completion.take() {
            if matches!(&completion, V2IoCompletion::LifecycleCertifiedServe(_)) {
                return IoCompletionTake::ready(PendingServiceCompletion::Io {
                    completion,
                    ownership_position: 0,
                });
            }
            self.held_io_completion = Some(completion);
            return IoCompletionTake::unavailable();
        }
        let Some(io) = self.io.as_ref() else {
            return IoCompletionTake::unavailable();
        };
        let Ok(completion) = io.try_recv_completion_unacknowledged() else {
            return IoCompletionTake::unavailable();
        };
        if matches!(&completion, V2IoCompletion::LifecycleCertifiedServe(_)) {
            IoCompletionTake::ready(PendingServiceCompletion::Io {
                completion,
                ownership_position: 0,
            })
        } else {
            self.held_io_completion = Some(completion);
            IoCompletionTake::unavailable()
        }
    }
    fn take_next_completion(&mut self, runtime_capacity_available: bool) -> IoCompletionTake {
        let completion = if runtime_capacity_available && self.held_io_completion.is_some() {
            // Once capacity returns, the exact runtime result which first
            // encountered backpressure precedes both later I/O and the local
            // reconstruction source.
            self.take_io_completion(true)
        } else {
            match self.next_completion_source {
                CompletionSource::Io => match self.take_io_completion(runtime_capacity_available) {
                    IoCompletionTake {
                        completion: None,
                        retained_runtime: false,
                    } if runtime_capacity_available => self
                        .local_completions
                        .front()
                        .cloned()
                        .map_or_else(IoCompletionTake::unavailable, |completion| {
                            IoCompletionTake::ready(PendingServiceCompletion::Local(completion))
                        }),
                    completion => completion,
                },
                CompletionSource::Local if runtime_capacity_available => {
                    self.local_completions.front().cloned().map_or_else(
                        || self.take_io_completion(true),
                        |completion| {
                            IoCompletionTake::ready(PendingServiceCompletion::Local(completion))
                        },
                    )
                }
                CompletionSource::Local => self.take_io_completion(false),
            }
        };
        if let Some(completion) = &completion.completion {
            self.next_completion_source = match completion {
                PendingServiceCompletion::Io { .. } => CompletionSource::Local,
                PendingServiceCompletion::Local(_) => CompletionSource::Io,
            };
        }
        completion
    }
    fn take_timeout_recovery_prefix_completion(
        &mut self,
        runtime_capacity_available: bool,
        inclusive_lifecycle_cut: u128,
    ) -> IoCompletionTake {
        self.take_lifecycle_prefix_completion(
            runtime_capacity_available,
            inclusive_lifecycle_cut,
            true,
        )
    }
    fn take_lifecycle_prefix_completion(
        &mut self,
        runtime_capacity_available: bool,
        lifecycle_cut: u128,
        inclusive: bool,
    ) -> IoCompletionTake {
        let within_cut = |ordinal: u128| {
            if inclusive {
                ordinal <= lifecycle_cut
            } else {
                ordinal < lifecycle_cut
            }
        };
        let ownership_position =
            usize::from(!runtime_capacity_available && self.held_io_completion.is_some());
        let io_ownership = self
            .io
            .as_ref()
            .and_then(|io| io.completion_ownership_at(ownership_position))
            .filter(|owned| {
                owned.runtime_lifecycle_ordinal.is_some_and(|ordinal| {
                    within_cut(ordinal)
                        && (runtime_capacity_available || !owned.requires_runtime_capacity)
                })
            });
        let local = if runtime_capacity_available {
            self.local_completions
                .iter()
                .filter(|completion| within_cut(completion.runtime_lifecycle_ordinal()))
                .min_by_key(|completion| completion.runtime_lifecycle_ordinal())
                .cloned()
        } else {
            None
        };
        let source = match (
            io_ownership.and_then(|owned| owned.runtime_lifecycle_ordinal),
            local
                .as_ref()
                .map(LocalCompletion::runtime_lifecycle_ordinal),
        ) {
            (Some(io), Some(local)) if io < local => Some(CompletionSource::Io),
            (Some(io), Some(local)) if local < io => Some(CompletionSource::Local),
            (Some(_), Some(_)) => Some(self.next_completion_source),
            (Some(_), None) => Some(CompletionSource::Io),
            (None, Some(_)) => Some(CompletionSource::Local),
            (None, None) => None,
        };
        let completion = match source {
            Some(CompletionSource::Io) => {
                let take = self.take_io_completion(runtime_capacity_available);
                if take.completion.is_none()
                    && !take.retained_runtime
                    && let Some(local) = local
                {
                    IoCompletionTake::ready(PendingServiceCompletion::Local(local))
                } else {
                    take
                }
            }
            Some(CompletionSource::Local) => IoCompletionTake::ready(
                PendingServiceCompletion::Local(local.expect("selected local completion exists")),
            ),
            None => IoCompletionTake::unavailable(),
        };
        if let Some(completion) = &completion.completion {
            self.next_completion_source = match completion {
                PendingServiceCompletion::Io { .. } => CompletionSource::Local,
                PendingServiceCompletion::Local(_) => CompletionSource::Io,
            };
        }
        completion
    }
    fn retire_held_io_completion(&mut self) {
        let Some(completion) = self.held_io_completion.take() else {
            return;
        };
        if matches!(
            &completion,
            V2IoCompletion::RecoveredLifecycleSign(_) | V2IoCompletion::LifecycleCertifiedServe(_)
        ) {
            // Dropping the armed completion closes output while `self.io`
            // still retains the dedicated queue/index owner. It must never be
            // acknowledged or removed by generic teardown.
            return;
        }
        if let Some(io) = self.io.as_ref() {
            io.acknowledge_completion(&completion)
                .expect("completion acknowledgement is infallible");
        }
    }
    /// Drain tagged I/O/reconstruction completions while runtime has capacity;
    /// backpressured responses transfer to exact output or remain reconstructible.
    pub(crate) fn drain_completions<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
    ) -> Result<usize, EffectExecutorError> {
        let outcome = self.drain_completions_with_lifecycle(executor)?;
        self.require_no_unowned_lifecycle_completion(executor, outcome)
    }
    /// Take and classify the oldest Completion-lane owner in one operation.
    ///
    /// This is the lifecycle driver's sole physical-head classifier. It does
    /// not probe three mutually exclusive drains. A pending local completion,
    /// or an ordinary I/O head, returns `PassThrough` without acknowledgement
    /// or ownership-position removal. A recovered result transfers exactly its
    /// dedicated guarded token and advances completion-source rotation once.
    pub(in crate::sumeragi) fn take_next_recovered_lifecycle_completion(
        &mut self,
    ) -> Result<RecoveredLifecycleCompletionTakeV1, String> {
        if self.output_guard.restart_required() {
            return Err("Sumeragi v2 consensus requires process restart".to_owned());
        }
        if self.held_io_completion.is_none()
            && self.next_completion_source == CompletionSource::Local
            && !self.local_completions.is_empty()
        {
            return Ok(RecoveredLifecycleCompletionTakeV1::PassThrough);
        }

        let completion = if let Some(completion) = self.held_io_completion.take() {
            completion
        } else {
            let Some(io) = self.io.as_ref() else {
                return Ok(RecoveredLifecycleCompletionTakeV1::None);
            };
            let Ok(completion) = io.try_recv_completion_unacknowledged() else {
                return if self.local_completions.is_empty() {
                    Ok(RecoveredLifecycleCompletionTakeV1::None)
                } else {
                    Ok(RecoveredLifecycleCompletionTakeV1::PassThrough)
                };
            };
            completion
        };

        match completion {
            V2IoCompletion::RecoveredDecisionApply(guarded) => {
                let key = guarded.result().dispatch_key();
                let work_ack = match self.io.as_ref().ok_or_else(|| {
                    "recovered Decision Apply completion lost its I/O service owner".to_owned()
                }) {
                    Ok(io) => match io
                        .prepare_recovered_decision_apply_ack(key, Arc::clone(&self.output_guard))
                    {
                        Ok(work_ack) => work_ack,
                        Err(error) => {
                            self.held_io_completion =
                                Some(V2IoCompletion::RecoveredDecisionApply(guarded));
                            return Err(error);
                        }
                    },
                    Err(error) => {
                        self.held_io_completion =
                            Some(V2IoCompletion::RecoveredDecisionApply(guarded));
                        return Err(error);
                    }
                };
                self.next_completion_source = CompletionSource::Local;
                Ok(RecoveredLifecycleCompletionTakeV1::Apply(
                    PreparedRecoveredDecisionApplyCompletionV1 { guarded, work_ack },
                ))
            }
            V2IoCompletion::RecoveredLifecycleSign(guarded) => {
                let completion = self
                    .io
                    .as_ref()
                    .and_then(|io| io.prepare_recovered_lifecycle_sign_completion(guarded, 0))
                    .ok_or_else(|| {
                        "recovered Sign completion lost its exact dedicated owner".to_owned()
                    })?;
                self.next_completion_source = CompletionSource::Local;
                Ok(RecoveredLifecycleCompletionTakeV1::Sign(completion))
            }
            V2IoCompletion::RecoveredDecisionFetchBodyPersisted(guarded) => {
                let completion = self
                    .io
                    .as_ref()
                    .and_then(|io| io.prepare_recovered_decision_fetch_body_completion(guarded, 0))
                    .ok_or_else(|| {
                        "recovered Decision Fetch body completion lost its exact dedicated owner"
                            .to_owned()
                    })?;
                self.next_completion_source = CompletionSource::Local;
                Ok(RecoveredLifecycleCompletionTakeV1::DecisionFetch(
                    completion,
                ))
            }
            V2IoCompletion::LifecycleCertifiedServe(guarded) => {
                let completion = self
                    .io
                    .as_ref()
                    .and_then(|io| io.prepare_lifecycle_certified_serve_completion(guarded, 0))
                    .ok_or_else(|| {
                        "lifecycle Certified-Serve completion lost its exact dedicated owner"
                            .to_owned()
                    })?;
                self.next_completion_source = CompletionSource::Local;
                Ok(RecoveredLifecycleCompletionTakeV1::CertifiedServe(
                    completion,
                ))
            }
            ordinary => {
                assert!(
                    self.held_io_completion.is_none(),
                    "ordinary pass-through must restore the sole held completion slot"
                );
                self.held_io_completion = Some(ordinary);
                Ok(RecoveredLifecycleCompletionTakeV1::PassThrough)
            }
        }
    }

    /// Drain only the oldest recovered-Sign guard; other heads remain parked and
    /// generic drains cannot acknowledge this completion.
    pub(in crate::sumeragi) fn drain_recovered_lifecycle_sign_completion(
        &mut self,
    ) -> Result<RecoveredLifecycleSignCompletionDrainV1, String> {
        if self.output_guard.restart_required() {
            return Err("Sumeragi v2 consensus requires process restart".to_owned());
        }
        let take = self.take_recovered_lifecycle_sign_completion();
        let Some(PendingServiceCompletion::Io {
            completion: V2IoCompletion::RecoveredLifecycleSign(guarded),
            ownership_position,
        }) = take.completion
        else {
            return Ok(RecoveredLifecycleSignCompletionDrainV1 { completion: None });
        };
        let completion = self
            .io
            .as_ref()
            .and_then(|io| {
                io.prepare_recovered_lifecycle_sign_completion(guarded, ownership_position)
            })
            .ok_or_else(|| "recovered Sign completion lost its exact dedicated owner".to_owned())?;
        Ok(RecoveredLifecycleSignCompletionDrainV1 {
            completion: Some(completion),
        })
    }
    /// Drain the oldest lifecycle Serve; restore every other head unchanged.
    pub(in crate::sumeragi) fn drain_lifecycle_certified_serve_completion(
        &mut self,
    ) -> Result<LifecycleCertifiedServeCompletionDrainV1, String> {
        if self.output_guard.restart_required() {
            return Err("Sumeragi v2 consensus requires process restart".to_owned());
        }
        let take = self.take_lifecycle_certified_serve_completion();
        let Some(PendingServiceCompletion::Io {
            completion: V2IoCompletion::LifecycleCertifiedServe(guarded),
            ownership_position,
        }) = take.completion
        else {
            return Ok(LifecycleCertifiedServeCompletionDrainV1 { completion: None });
        };
        let completion = self
            .io
            .as_ref()
            .and_then(|io| {
                io.prepare_lifecycle_certified_serve_completion(guarded, ownership_position)
            })
            .ok_or_else(|| {
                "lifecycle Certified-Serve completion lost its exact dedicated owner".to_owned()
            })?;
        Ok(LifecycleCertifiedServeCompletionDrainV1 {
            completion: Some(completion),
        })
    }
    /// Drain the ordinary bounded completion source while returning a
    /// persisted certified-Fetch body directly to its serialized owner.
    ///
    /// TODO: Give the final runner one `LifecycleCoordinator`/registry owner and
    /// consume this typed outcome only after restart recovery can rebuild the
    /// exact Ready-Fetch response occurrence from a typed durable locator (or
    /// this transaction durably advances directly to the BodyFrame-bound Store
    /// stage). Until then, the count-only caller fail-stops on this outcome.
    pub(crate) fn drain_completions_with_lifecycle<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
    ) -> Result<V2CompletionDrainOutcome, EffectExecutorError> {
        self.drain_completions_inner(
            executor,
            MAX_COMPLETION_DRAIN_BATCH,
            CompletionDrainPolicy::Fair,
        )
    }
    /// Admit one completed owner from the inclusive timeout prefix (`<=`), while
    /// fresh producers receive larger ordinals behind the retained response.
    pub(crate) fn drain_timeout_recovery_prefix_completion<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
        inclusive_lifecycle_cut: u128,
    ) -> Result<usize, EffectExecutorError> {
        let outcome = self.drain_completions_inner(
            executor,
            1,
            CompletionDrainPolicy::TimeoutRecoveryPrefix {
                inclusive_lifecycle_cut,
            },
        )?;
        self.require_no_unowned_lifecycle_completion(executor, outcome)
    }
    fn drain_completions_inner<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
        limit: usize,
        policy: CompletionDrainPolicy,
    ) -> Result<V2CompletionDrainOutcome, EffectExecutorError> {
        if self.output_guard.restart_required() {
            return Err(executor
                .external_service_failed("Sumeragi v2 consensus requires process restart", self));
        }
        let mut count = 0usize;
        let mut attempts = 0usize;
        let mut worker_completion_deferred = false;
        let mut local_completion_deferred = false;
        let mut certified_fetch_body = None;
        while attempts < limit {
            let runtime_capacity_available = executor.remaining_completion_capacity() != 0;
            let take = match policy {
                CompletionDrainPolicy::Fair => {
                    self.take_next_completion(runtime_capacity_available)
                }
                CompletionDrainPolicy::TimeoutRecoveryPrefix {
                    inclusive_lifecycle_cut,
                } => self.take_timeout_recovery_prefix_completion(
                    runtime_capacity_available,
                    inclusive_lifecycle_cut,
                ),
            };
            let completion = match take.completion {
                Some(completion) => completion,
                None if take.retained_runtime => {
                    attempts = attempts.saturating_add(1);
                    if !worker_completion_deferred {
                        worker_completion_deferred = self
                            .io
                            .as_ref()
                            .is_some_and(|io| io.record_completion_service_attempt(0));
                    }
                    continue;
                }
                None => {
                    if !runtime_capacity_available
                        && !worker_completion_deferred
                        && (self.held_io_completion.is_some()
                            || self.io.as_ref().is_some_and(|io| {
                                io.completion_requires_runtime_capacity_at(0) == Some(true)
                            }))
                    {
                        worker_completion_deferred = self
                            .io
                            .as_ref()
                            .is_some_and(|io| io.record_completion_service_attempt(0));
                    }
                    break;
                }
            };
            attempts = attempts.saturating_add(1);
            let io_acknowledgement = match &completion {
                PendingServiceCompletion::Io {
                    completion,
                    ownership_position,
                } => Some((completion.acknowledgement(), *ownership_position)),
                PendingServiceCompletion::Local(_) => None,
            };
            let mut certified_fetch_work_ack = match &completion {
                PendingServiceCompletion::Io {
                    completion: V2IoCompletion::CertifiedFetchBodyPersisted(completion),
                    ..
                } => {
                    let prepared = self.io.as_ref().map_or_else(
                        || {
                            Err("persisted certified-Fetch body lost its I/O command owner"
                                .to_owned())
                        },
                        |io| {
                            io.prepare_certified_fetch_body_persistence_ack(
                                completion.completion(),
                                Arc::clone(&self.output_guard),
                            )
                        },
                    );
                    match prepared {
                        Ok(prepared) => Some(prepared),
                        Err(reason) => {
                            return Err(executor.external_service_failed(reason, self));
                        }
                    }
                }
                PendingServiceCompletion::Io { .. } | PendingServiceCompletion::Local(_) => None,
            };
            let serviced: Result<(), EffectExecutorError> = (|| {
                match completion {
                    PendingServiceCompletion::Io {
                        completion:
                            V2IoCompletion::Signature {
                                work_id,
                                signature,
                                outbound_payload,
                            },
                        ..
                    } => {
                        let disposition =
                            executor.complete_consensus_signature(work_id, signature, self)?;
                        if let Err(reason) = self
                            .restore_outbound_payload_after_signature(disposition, outbound_payload)
                        {
                            return Err(executor.external_service_failed(reason, self));
                        }
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::Stored(completion),
                        ..
                    } => {
                        let stored_subject = completion.manifest().subject;
                        let _ = executor.complete_body_store(completion, self)?;
                        if let Err(reason) = self.retry_locked_candidate_after_store(stored_subject)
                        {
                            return Err(executor.external_service_failed(reason, self));
                        }
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::CertifiedFetchBodyPersisted(completion),
                        ..
                    } => {
                        assert!(
                            certified_fetch_body.is_none(),
                            "one bounded drain turn returns at most one lifecycle completion"
                        );
                        certified_fetch_body =
                            Some(PreparedCertifiedFetchBodyPersistenceCompletion {
                                completion: completion.into_completion(),
                                work_ack: certified_fetch_work_ack.take().expect(
                                    "persisted Fetch completion retains its exact work ack",
                                ),
                            });
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::Validated(completion),
                        ..
                    } => {
                        let _ = executor.complete_body_validation(completion, self)?;
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::Applied(completion),
                        ..
                    } => {
                        let source_height = completion.artifact().height;
                        let source_block_hash = completion.artifact().block_hash;
                        let disposition = executor.complete_application(*completion, self)?;
                        if disposition == CompletionDisposition::Accepted {
                            self.kura_replica_advert_refresh
                                .note_durable_tip(
                                    Some((source_height, source_block_hash)),
                                    Instant::now(),
                                )
                                .map_err(|reason| executor.external_service_failed(reason, self))?;
                        }
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::ApplyDeferred { work_id, reference },
                        ..
                    } => {
                        let _ = executor
                            .defer_application_for_merge_sidecar(work_id, &reference, self)?;
                    }
                    #[cfg(test)]
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::AuxiliaryNoop,
                        ..
                    } => {}
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::CandidateLoaded(candidate),
                        ..
                    } => {
                        if !self.proposal_work_retired {
                            let subject = candidate.subject;
                            let tag = match self.complete_locked_candidate_load(candidate) {
                                Ok(tag) => tag,
                                Err(reason) => {
                                    return Err(executor.external_service_failed(reason, self));
                                }
                            };
                            if let Some(tag) = tag {
                                iroha_logger::debug!(
                                    height = tag.height(),
                                    view = tag.view(),
                                    generation = tag.generation().get(),
                                    ?subject,
                                    "loaded exact locked body for Sumeragi v2 re-proposal"
                                );
                            } else {
                                iroha_logger::debug!(
                                    ?subject,
                                    "retired superseded locked-body load before Sumeragi v2 re-proposal"
                                );
                            }
                        }
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::RecoveredDecisionApply(_),
                        ..
                    } => {
                        return Err(executor.external_service_failed(
                            "recovered Decision Apply completion crossed the generic executor drain",
                            self,
                        ));
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::RecoveredLifecycleSign(_),
                        ..
                    } => {
                        return Err(executor.external_service_failed(
                            "recovered Sign completion crossed the generic executor drain",
                            self,
                        ));
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::RecoveredDecisionFetchBodyPersisted(_),
                        ..
                    } => {
                        return Err(executor.external_service_failed(
                            "recovered Decision Fetch body crossed the generic executor drain",
                            self,
                        ));
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::LifecycleCertifiedServe(_),
                        ..
                    } => {
                        return Err(executor.external_service_failed(
                            "lifecycle Certified-Serve completion crossed the generic executor drain",
                            self,
                        ));
                    }
                    PendingServiceCompletion::Io {
                        completion:
                            V2IoCompletion::CandidateLoadUnavailable {
                                acquisition_id,
                                subject,
                            },
                        ..
                    } => {
                        if !self.proposal_work_retired {
                            if let Err(reason) =
                                self.locked_candidate_load_unavailable(acquisition_id, subject)
                            {
                                return Err(executor.external_service_failed(reason, self));
                            }
                            iroha_logger::debug!(
                                ?subject,
                                "locked Sumeragi v2 body is not durable yet; waiting for body-store recovery"
                            );
                        }
                    }
                    PendingServiceCompletion::Io {
                        completion:
                            V2IoCompletion::CandidateLoadFailed {
                                acquisition_id,
                                subject,
                                reason,
                            },
                        ..
                    } => {
                        if !self.proposal_work_retired {
                            if let Err(reason) =
                                self.locked_candidate_load_failed(acquisition_id, subject, reason)
                            {
                                return Err(executor.external_service_failed(reason, self));
                            }
                            iroha_logger::debug!(
                                ?subject,
                                "retired failed superseded locked-body load before Sumeragi v2 re-proposal"
                            );
                        }
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::Failed(reason),
                        ..
                    } => {
                        return Err(executor.external_service_failed(reason, self));
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::Retired,
                        ..
                    } => {
                        return Err(executor.external_service_failed(
                            "unexpected early Sumeragi v2 storage retirement",
                            self,
                        ));
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::RetirementFailed(reason),
                        ..
                    } => {
                        return Err(executor.external_service_failed(
                            format!(
                                "unexpected early Sumeragi v2 storage retirement failure: {reason}"
                            ),
                            self,
                        ));
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::RecoveryRequired(reason),
                        ..
                    } => {
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
                        match executor.complete_body_reconstruction(&task, manifest, body, self) {
                            Ok(CompletionDisposition::Rejected) => {
                                iroha_logger::debug!(
                                    work_id = task.id().get(),
                                    "rejected noncanonical reconstructed Sumeragi v2 body"
                                );
                            }
                            Ok(_) => {}
                            Err(EffectTransportError::Backpressure) => {
                                local_completion_deferred = true;
                            }
                            Err(error) => {
                                return Err(executor.external_service_failed(error, self));
                            }
                        }
                    }
                }
                Ok(())
            })();
            if let Some((acknowledgement, ownership_position)) = io_acknowledgement {
                let acknowledge = match &acknowledgement {
                    V2IoCompletionAcknowledgement::RecoveredLifecycleSignRetained
                    | V2IoCompletionAcknowledgement::RecoveredDecisionFetchRetained
                    | V2IoCompletionAcknowledgement::LifecycleServeRetained => false,
                    V2IoCompletionAcknowledgement::Work(_)
                    | V2IoCompletionAcknowledgement::LifecycleWorkRetained
                    | V2IoCompletionAcknowledgement::RecoveredDecisionApplyRetained
                    | V2IoCompletionAcknowledgement::Untracked => true,
                };
                if acknowledge && let Some(io) = self.io.as_ref() {
                    let acknowledged =
                        io.acknowledge_completion_at(acknowledgement, ownership_position);
                    if let Err(reason) = acknowledged {
                        return Err(executor.external_service_failed(reason, self));
                    }
                }
            }
            serviced?;
            if local_completion_deferred {
                worker_completion_deferred = true;
                break;
            }
            count = count.saturating_add(1);
            if certified_fetch_body.is_some() {
                break;
            }
        }
        if count != 0 || worker_completion_deferred {
            let status = executor.status();
            if executor.remaining_completion_capacity() == 0
                && (status.pending_signatures != 0
                    || status.pending_fetches != 0
                    || status.pending_stores != 0
                    || status.pending_validations != 0
                    || status.pending_applications != 0
                    || !self.local_completions.is_empty()
                    || self.held_io_completion.is_some())
            {
                iroha_logger::debug!(
                    queued_runtime_commands = status.queued_runtime_completions,
                    pending_signatures = status.pending_signatures,
                    pending_fetches = status.pending_fetches,
                    pending_stores = status.pending_stores,
                    pending_validations = status.pending_validations,
                    pending_applications = status.pending_applications,
                    local_completions = self.local_completions.len(),
                    held_io_completion = self.held_io_completion.is_some(),
                    "deferred Sumeragi v2 service completion behind a full runtime FIFO"
                );
            }
            if let Err(reason) = self.publish_effect_status(&status) {
                return Err(executor.external_service_failed(reason, self));
            }
        }
        Ok(V2CompletionDrainOutcome {
            serviced: count,
            certified_fetch_body,
        })
    }
    fn require_no_unowned_lifecycle_completion<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
        outcome: V2CompletionDrainOutcome,
    ) -> Result<usize, EffectExecutorError> {
        let (serviced, completion) = outcome.into_parts();
        if completion.is_some() {
            return Err(executor.external_service_failed(
                "persisted certified-Fetch body has no live lifecycle coordinator owner",
                self,
            ));
        }
        Ok(serviced)
    }
    /// After Kura receipt verification, hand cleanup to the bounded janitor;
    /// failures retain files for reconciliation without delaying successors.
    pub(crate) fn finish_height(
        mut self,
        receipt: KuraV2CommitReceipt,
        cleanup_timeout: Duration,
        supervisor: &mut V2CleanupSupervisor,
    ) -> PostFinalityCleanupOutcome {
        let mut outcome = PostFinalityCleanupOutcome::default();
        let incomplete_exact_output_handoff = match self.pending_exact_output.lock() {
            Ok(_) if !self.exact_output_handoff_owner.is_sealed() => {
                Some("durable exact-output handoff was not sealed before finalized cleanup")
            }
            Ok(pending) if pending.is_pending() => {
                Some("durable exact-output handoff was sealed with pending output")
            }
            Ok(_) => None,
            Err(_) => {
                Some("durable exact-output corridor lock was poisoned before finalized cleanup")
            }
        };
        if let Some(reason) = incomplete_exact_output_handoff {
            outcome.record(PostFinalityCleanupTarget::CleanupWorker, reason);
            self.output_guard.activate_restart_required();
        } else {
            self.clean_teardown = true;
        }
        let deadline = Instant::now()
            .checked_add(cleanup_timeout)
            .unwrap_or_else(Instant::now);
        self.retire_held_io_completion();
        if let Some(mut io) = self.io.take() {
            let mut command = V2IoCommand::Retire(V2RetireCommand {
                receipt,
                cleanup: supervisor.submission(),
                chunk_root: self.chunk_root.clone(),
            });
            let retirement_guard = Arc::clone(&self.output_guard);
            'enqueue: loop {
                let Some(retirement_enqueue_permit) = retirement_guard.acquire() else {
                    outcome.record(
                        PostFinalityCleanupTarget::CleanupWorker,
                        "process restart became required before body retirement enqueue",
                    );
                    break;
                };
                let enqueue = io.try_enqueue(command);
                // Waiting for an older completion while holding this permit
                // would prevent fatal activation from draining output.
                drop(retirement_enqueue_permit);
                match enqueue {
                    Ok(()) => break,
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
                                break 'enqueue;
                            }
                            Ok(V2IoCompletion::RetirementFailed(reason)) => {
                                outcome.record(
                                    PostFinalityCleanupTarget::CleanupWorker,
                                    "Sumeragi v2 I/O worker reported body retirement failure",
                                );
                                outcome.record(PostFinalityCleanupTarget::DurableBodies, reason);
                                break 'enqueue;
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
                                break 'enqueue;
                            }
                            Err(CleanupCompletionWaitError::Disconnected) => {
                                outcome.record(
                                    PostFinalityCleanupTarget::CleanupWorker,
                                    "Sumeragi v2 I/O worker disconnected before body retirement",
                                );
                                break 'enqueue;
                            }
                        }
                    }
                    Err(V2IoTrySendError::Disconnected(_)) => {
                        outcome.record(
                            PostFinalityCleanupTarget::CleanupWorker,
                            "Sumeragi v2 I/O worker disconnected before body retirement",
                        );
                        break;
                    }
                    Err(
                        V2IoTrySendError::ConflictingWorkId { .. }
                        | V2IoTrySendError::UnreservedRecoveredDecisionApply { .. },
                    ) => {
                        unreachable!("retirement commands do not carry work identifiers")
                    }
                }
            }
            let join = io.join.take();
            // A successfully accepted Retire moves all blocking filesystem
            // work to the one runner-lifetime janitor before this worker
            // exits. Never join a running context worker on the consensus
            // thread; dropping its handle only detaches the already-closing
            // worker and cannot create another cleanup thread.
            drop(io);
            if let Some(join) = join {
                if join.is_finished() && join.join().is_err() {
                    outcome.record(
                        PostFinalityCleanupTarget::CleanupWorker,
                        "Sumeragi v2 I/O worker panicked during finalized cleanup",
                    );
                }
            }
        } else {
            outcome.record(
                PostFinalityCleanupTarget::CleanupWorker,
                "Sumeragi v2 I/O worker was unavailable for cleanup handoff",
            );
        }
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
    fn lock_pending_exact_output(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, PendingExactOutput>, String> {
        self.pending_exact_output
            .lock()
            .map_err(|_| "Sumeragi v2 outbound corridor lock was poisoned".to_owned())
    }
    /// Replace actor admission with a deterministic recoverable test boundary.
    #[cfg(test)]
    pub(in crate::sumeragi) fn set_exact_output_admission_hook(
        &mut self,
        mut hook: impl FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
        ) -> Result<(), NetworkActorAdmissionError<Post<NetworkMessage>>>
        + Send
        + 'static,
    ) {
        self.exact_output_admission_hook = Some(Mutex::new(Box::new(move |post, ticket| {
            hook(post, ticket).map(|()| ExactOutputTestAdmission::Admitted)
        })));
    }
    /// Replace reply admission with a controllable writer-flush test boundary.
    #[cfg(test)]
    pub(in crate::sumeragi) fn set_exact_output_flush_admission_hook(
        &mut self,
        hook: impl FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
        ) -> Result<
            ExactOutputTestAdmission,
            NetworkActorAdmissionError<Post<NetworkMessage>>,
        > + Send
        + 'static,
    ) {
        self.exact_output_admission_hook = Some(Mutex::new(Box::new(hook)));
    }
    /// Replace an empty exact-output corridor with a small production-shaped test geometry.
    #[cfg(test)]
    pub(in crate::sumeragi) fn set_exact_output_shared_unit_capacity_for_test(
        &self,
        shared_ownership_unit_capacity: usize,
    ) -> Result<(), String> {
        let max_messages_per_fanout = usize::try_from(self.context.da_layout.max_chunk_count)
            .map_err(|_| "Sumeragi v2 test outbound chunk count is not representable".to_owned())?
            .checked_add(1)
            .ok_or_else(|| "Sumeragi v2 test outbound fanout bound overflowed".to_owned())?;
        let max_peers_per_fanout = self
            .context
            .roster
            .len()
            .max(self.network.reply_route_source_capacity())
            .max(1);
        let frozen_semantic_targets = self
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let replacement = PendingExactOutput::new(
            shared_ownership_unit_capacity,
            max_messages_per_fanout,
            max_peers_per_fanout,
            &frozen_semantic_targets,
        )?;
        let mut pending = self.lock_pending_exact_output()?;
        if !pending.fanouts.is_empty() || !pending.admitted_sidecar_chunks.is_empty() {
            return Err("cannot replace a non-empty Sumeragi v2 exact-output corridor".to_owned());
        }
        *pending = replacement;
        Ok(())
    }
    /// Test whether the exact-output corridor retained a particular opaque
    /// reply tenure after a production service handoff.
    #[cfg(test)]
    pub(in crate::sumeragi) fn retains_reply_route_for_test(
        &self,
        expected: &NetworkReplyRoute,
    ) -> Result<bool, String> {
        self.lock_pending_exact_output().map(|pending| {
            pending.fanouts.iter().any(|fanout| {
                fanout.targets.iter().any(|target| {
                    matches!(
                        &target.route,
                        ExactTargetRoute::Reply(route) if route.same_tenure(expected)
                    )
                })
            })
        })
    }
    #[cfg(test)]
    /// Return whether fail-stop output handling requires a process restart.
    pub(in crate::sumeragi) fn exact_output_restart_required_for_test(&self) -> bool {
        self.output_guard.restart_required()
    }
    /// Hold one auxiliary I/O unit without fabricating a queue command.
    #[cfg(test)]
    pub(in crate::sumeragi) fn hold_auxiliary_io_admission_for_test(
        &self,
    ) -> Result<ProductionAuxiliaryIoAdmissionHoldV1, String> {
        let io = self
            .io
            .as_ref()
            .ok_or_else(|| "Sumeragi v2 I/O worker is unavailable".to_owned())?;
        if !io.admission.try_reserve(V2IoAdmissionClass::Auxiliary) {
            return Err("Sumeragi v2 auxiliary I/O admission is full".to_owned());
        }
        Ok(ProductionAuxiliaryIoAdmissionHoldV1 {
            admission: Arc::clone(&io.admission),
        })
    }

    fn admit_network_exact_output(
        &self,
        post: Post<NetworkMessage>,
        ticket: Option<NetworkActorAdmissionTicket>,
        route: &ExactTargetRoute,
        reply_writer_timeout_attempt: u8,
    ) -> Result<ExactOutputAttemptOutcome, NetworkActorAdmissionError<Post<NetworkMessage>>> {
        match route {
            ExactTargetRoute::Topology => self
                .network
                .post_recoverable(post, ticket)
                .map(|()| ExactOutputAttemptOutcome::Admitted),
            ExactTargetRoute::Reply(reply_route) => {
                let requires_sidecar_flush = matches!(
                    &post.data,
                    NetworkMessage::CertifiedMergeSidecar(message)
                        if matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(_))
                );
                match self
                    .network
                    .post_reply_recoverable_with_flush_ack_at_attempt(
                        post,
                        reply_route,
                        ticket,
                        reply_writer_timeout_attempt,
                    )? {
                    Some(flush_ack) if requires_sidecar_flush => {
                        Ok(ExactOutputAttemptOutcome::SidecarFlush(flush_ack))
                    }
                    Some(flush_ack) => Ok(ExactOutputAttemptOutcome::ReplyFlush(flush_ack)),
                    None if reply_route.is_active() && !reply_route.is_reply_writable() => {
                        Ok(ExactOutputAttemptOutcome::Unavailable)
                    }
                    None => Ok(ExactOutputAttemptOutcome::Retired),
                }
            }
        }
    }
    fn drive_pending_exact_output(&self, pending: &mut PendingExactOutput) -> Result<bool, String> {
        pending.poll_reply_flushes()?;
        let outcome = {
            #[cfg(test)]
            {
                if let Some(hook) = &self.exact_output_admission_hook {
                    let mut hook = hook.lock().map_err(|_| {
                        "Sumeragi v2 exact-output admission hook was poisoned".to_owned()
                    })?;
                    pending.drive_bounded_with_ack(|post, ticket, route, _timeout_attempt| {
                        hook(post, ticket).map(|outcome| match outcome {
                            ExactOutputTestAdmission::Admitted
                                if matches!(route, ExactTargetRoute::Reply(_)) =>
                            {
                                ExactOutputAttemptOutcome::TestReplyFlushed
                            }
                            ExactOutputTestAdmission::Admitted => {
                                ExactOutputAttemptOutcome::Admitted
                            }
                            ExactOutputTestAdmission::SidecarFlush(flush_ack) => {
                                ExactOutputAttemptOutcome::SidecarFlush(flush_ack)
                            }
                            ExactOutputTestAdmission::Retired => ExactOutputAttemptOutcome::Retired,
                        })
                    })?
                } else {
                    pending.drive_bounded_with_ack(|post, ticket, route, timeout_attempt| {
                        self.admit_network_exact_output(post, ticket, route, timeout_attempt)
                    })?
                }
            }
            #[cfg(not(test))]
            {
                pending.drive_bounded_with_ack(|post, ticket, route, timeout_attempt| {
                    self.admit_network_exact_output(post, ticket, route, timeout_attempt)
                })?
            }
        };
        pending.poll_reply_flushes()?;
        match outcome {
            ExactOutputDriveOutcome::Drained => {}
            ExactOutputDriveOutcome::ReceiptBackpressured => {
                iroha_logger::debug!(
                    pending_receipts = pending.sidecar_control_units(),
                    pending_flushes = pending.pending_sidecar_flushes(),
                    receipt_capacity = pending.sidecar_admission_capacity,
                    "retained exact Sumeragi v2 output behind sidecar receipt backpressure"
                );
            }
            ExactOutputDriveOutcome::Backpressured { closest_rank } => {
                iroha_logger::debug!(
                    rank = closest_rank,
                    pending_fanouts = pending.fanouts.len(),
                    "retained exact Sumeragi v2 output behind network-actor backpressure"
                );
            }
            ExactOutputDriveOutcome::BudgetExhausted {
                closest_backpressure_rank,
            } => {
                iroha_logger::debug!(
                    rank = ?closest_backpressure_rank,
                    pending_fanouts = pending.fanouts.len(),
                    attempt_budget = pending.drive_attempt_budget,
                    "yielded a bounded exact Sumeragi v2 output admission slice"
                );
            }
        }
        Ok(pending.is_pending())
    }
    fn enqueue_exact_fanout_while_guarded(
        &self,
        messages: Vec<NetworkMessage>,
        peers: Vec<PeerId>,
        rollover_claim: ExactOutputRolloverClaim,
        _permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        let Some(fanout) = PendingExactFanout::claimed(messages, peers, rollover_claim)? else {
            return Ok(ExactFanoutOwnership::Owned);
        };
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            return Err(
                "Sumeragi v2 exact output is sealed after durable finality handoff".to_owned(),
            );
        }
        let ownership = pending.enqueue(fanout)?;
        if ownership == ExactFanoutOwnership::Owned {
            let _ = self.drive_pending_exact_output(&mut pending)?;
        }
        Ok(ownership)
    }
    /// Transfer an inseparable topology batch after same-lock bound/capacity/FIFO
    /// checks, returning it whole when full.
    fn enqueue_atomic_fanout_batch_while_guarded(
        &self,
        fanouts: Vec<PendingExactFanout>,
        _permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            return Err(
                "Sumeragi v2 exact output is sealed after durable finality handoff".to_owned(),
            );
        }
        let Some(batch) = pending.prepare_atomic_fanout_batch(fanouts)? else {
            return Ok(ExactFanoutOwnership::SourceRetained);
        };
        pending.commit_atomic_fanout_batch(batch);
        let _ = self.drive_pending_exact_output(&mut pending)?;
        Ok(ExactFanoutOwnership::Owned)
    }
    fn enqueue_owned_exact_reply_routes_while_guarded(
        &self,
        message: NetworkMessage,
        peer: PeerId,
        reply_routes: NetworkReplyRoutes,
        ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
        rollover_claim: ExactOutputRolloverClaim,
        _permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        if reply_routes.semantic_target() != &peer {
            return Err(
                "Sumeragi v2 reply route does not match its semantic output target".to_owned(),
            );
        }
        let Some(fanout) = PendingExactFanout::claimed_with_reply_routes_and_ingress_ownership(
            vec![message],
            peer,
            reply_routes,
            ingress_ownership,
            rollover_claim,
        )?
        else {
            return Ok(ExactFanoutOwnership::Owned);
        };
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            return Err(
                "Sumeragi v2 exact output is sealed after durable finality handoff".to_owned(),
            );
        }
        let ownership = pending.enqueue_owned_reply_transfer(fanout)?;
        if ownership == ExactFanoutOwnership::Owned {
            let _ = self.drive_pending_exact_output(&mut pending)?;
        }
        Ok(ownership)
    }
    fn exact_output_scope(&self) -> ExactOutputCreationScope {
        ExactOutputCreationScope {
            context_id: self.context.id(),
            height: self.context.height,
        }
    }
    /// Advance the shared process-lifetime advert refresher by one bounded
    /// turn.  A retained refresh token is independent of `PendingExactOutput`;
    /// only an accepted enqueue gains an exact rollover claim.
    pub(crate) fn service_kura_replica_advert_refresh_turn(
        &self,
        now: Instant,
    ) -> Result<KuraReplicaAdvertRefreshTurnOutcome, String> {
        if self.exact_output_handoff_owner.is_sealed() {
            return Ok(KuraReplicaAdvertRefreshTurnOutcome::default());
        }
        let durable_tip = self
            .kura
            .exact_kura_replica_advert_tip()
            .map_err(|error| error.to_string())?;
        self.kura_replica_advert_refresh
            .note_durable_tip(durable_tip, now)?;
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        let outcome = self.kura_replica_advert_refresh.drive_turn(
            now,
            |source_height| {
                self.kura
                    .probe_kura_replica_advert_source(source_height, &self.key_pair)
                    .map_err(|error| error.to_string())
            },
            |source| self.post_kura_replica_advert_while_guarded(source, operation.permit()),
        )?;
        operation.complete();
        Ok(outcome)
    }
    /// Retry every currently schedulable exact semantic-output target.
    ///
    /// Returns `true` while an exact actor-backpressured target remains owned.
    pub(crate) fn retry_pending_exact_output(&self) -> Result<bool, String> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        let pending_remains = {
            let mut pending = self.lock_pending_exact_output()?;
            if self.exact_output_handoff_owner.is_sealed() {
                debug_assert!(!pending.is_pending());
                operation.complete();
                return Ok(false);
            }
            self.drive_pending_exact_output(&mut pending)?
        };
        operation.complete();
        Ok(pending_remains)
    }
    /// After exact Kura/finality authority, transfer finalized height-local,
    /// durable lane, Kura-backed response, and exact-scope sidecar output to
    /// reconstruction; manual or cross-scope output stays owned.
    pub(crate) fn handoff_applied_height_output_to_durable_reconstruction(
        &self,
        receipt: &KuraV2CommitReceipt,
        artifact: &wire::finality::V2FinalityArtifact,
        durable_lane_authority: &DurableLaneRolloverAuthority,
    ) -> Result<usize, String> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        self.validate_applied_height_output_handoff_authority(receipt, artifact)?;
        let (retired, retired_kura_replica_advert_heights) = {
            let mut pending = self.lock_pending_exact_output()?;
            if self.exact_output_handoff_owner.is_sealed() {
                return Err(
                    "Sumeragi v2 applied-height output handoff is already sealed".to_owned(),
                );
            }
            let retired_kura_replica_advert_heights =
                pending.pending_kura_replica_advert_heights()?;
            let retired = pending.handoff_applied_height_to_durable_reconstruction(
                artifact,
                Some(durable_lane_authority),
                Some(self.kura.as_ref()),
            )?;
            (retired, retired_kura_replica_advert_heights)
        };
        let scheduled_kura_replica_adverts = self
            .kura_replica_advert_refresh
            .schedule_retired_exact_output_heights(
                retired_kura_replica_advert_heights,
                Instant::now(),
            )?;
        if retired != 0 {
            iroha_logger::debug!(
                height = receipt.height(),
                retired_posts = retired,
                scheduled_kura_replica_adverts,
                "handed backpressured finalized-height output to durable reconstruction"
            );
        }
        operation.complete();
        Ok(retired)
    }
    /// After lane handoff quiesces, revalidate authority, perform the final
    /// atomic handoff, require emptiness, seal enqueue, and mint one receipt.
    pub(crate) fn seal_applied_height_output_handoff(
        &self,
        receipt: &KuraV2CommitReceipt,
        artifact: &wire::finality::V2FinalityArtifact,
        durable_lane_authority: &DurableLaneRolloverAuthority,
    ) -> Result<DurableExactOutputHandoffReceipt, String> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        self.validate_applied_height_output_handoff_authority(receipt, artifact)?;
        let retired = {
            let mut pending = self.lock_pending_exact_output()?;
            let retired = pending.handoff_applied_height_to_durable_reconstruction(
                artifact,
                Some(durable_lane_authority),
                Some(self.kura.as_ref()),
            )?;
            if pending.is_pending() {
                return Err(
                    "Sumeragi v2 final exact-output handoff did not clear its corridor".to_owned(),
                );
            }
            if retired != 0 {
                return Err(
                    "Sumeragi v2 final exact-output seal observed newly retained output".to_owned(),
                );
            }
            self.exact_output_handoff_owner.seal()?;
            retired
        };
        debug_assert_eq!(retired, 0);
        let handoff = DurableExactOutputHandoffReceipt {
            owner: Arc::clone(&self.exact_output_handoff_owner.0),
            predecessor_context_hash: HashOf::new(&self.context),
            predecessor_context_id: self.context.id(),
            predecessor_height: self.context.height,
            predecessor_network_id: self.context.network_id,
            finality_artifact_hash: HashOf::new(artifact),
            finality_commit_qc: artifact.commit_qc.clone(),
        };
        operation.complete();
        Ok(handoff)
    }
    fn validate_applied_height_output_handoff_authority(
        &self,
        receipt: &KuraV2CommitReceipt,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> Result<(), String> {
        artifact.validate().map_err(|error| error.to_string())?;
        if artifact.height_context != self.context
            || receipt.height() != self.context.height
            || receipt.context_id() != self.context.id()
            || receipt.subject() != artifact.subject
            || receipt.block_hash() != artifact.block_hash
            || receipt.certificate() != artifact.commit_qc.as_ref()
            || receipt.artifact_hash() != HashOf::new(artifact)
        {
            return Err(
                "Sumeragi v2 applied-height output handoff has mismatched finality authority"
                    .to_owned(),
            );
        }
        Ok(())
    }
    /// Drain process-local sidecar receipts after the exact peer writer flushes
    /// their response chunks.
    pub(crate) fn drain_certified_merge_sidecar_chunk_admissions(
        &self,
        limit: usize,
    ) -> Result<Vec<CertifiedMergeSidecarChunkAdmission>, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Ok(Vec::new());
        }
        pending.poll_reply_flushes()?;
        let count = limit.min(pending.admitted_sidecar_chunks.len());
        Ok(pending.admitted_sidecar_chunks.drain(..count).collect())
    }
    /// Cancel every queued or writer-pending response occurrence covered by an
    /// authenticated cumulative close for the exact durable stream incarnation
    /// before any newer output is dispatched.
    pub(crate) fn close_certified_merge_sidecar_prefix(
        &self,
        prefix: &CertifiedMergeSidecarClosedPrefix,
    ) -> Result<usize, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Ok(0);
        }
        pending.close_certified_sidecar_prefix(prefix)
    }
    /// Cancel every exact-output occurrence whose historical request owner
    /// completed through another authenticated source.
    pub(crate) fn cancel_historical_lane_recovery_requests(
        &self,
        request_hashes: &BTreeSet<HashOf<LaneHistoricalRecoveryRequestV1>>,
    ) -> Result<usize, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Ok(0);
        }
        pending.cancel_historical_lane_recovery_requests(request_hashes)
    }
    /// Cancel requester-side sidecar output after its transport attempt retires.
    pub(crate) fn cancel_certified_merge_sidecar_requests(
        &self,
        request_hashes: &BTreeSet<HashOf<CertifiedMergeSidecarRequestV1>>,
    ) -> Result<usize, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Ok(0);
        }
        pending.cancel_certified_merge_sidecar_requests(request_hashes)
    }
    /// Cancel requester-side Close retries covered by cumulative acknowledgements.
    pub(crate) fn cancel_acknowledged_certified_merge_sidecar_closes(
        &self,
        acknowledgements: &[CertifiedMergeSidecarCloseAckV1],
    ) -> Result<usize, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Ok(0);
        }
        pending.cancel_acknowledged_certified_merge_sidecar_closes(acknowledgements)
    }
    /// Check the exact target/class/kind reservation for the next lane-work effect.
    pub(crate) fn can_retain_lane_work_effect_from_snapshot(
        &self,
        effect: &V2LaneWorkEffect,
        queue_plan_sources: Option<&mut QueuePlanBatchSources>,
    ) -> Result<bool, String> {
        let (messages, peers, routes, reply_route_history, ingress_ownership, rollover_claim) =
            match effect {
                V2LaneWorkEffect::PostLaneBlock { peer, message } => {
                    let rollover_claim = match message {
                        BlockMessage::LaneHistoricalRecoveryRequest(request) => {
                            ExactOutputRolloverClaim::HistoricalLaneRecoveryRequest {
                                scope: self.exact_output_scope(),
                                target: peer.clone(),
                                request_hash: HashOf::new(request.as_ref()),
                            }
                        }
                        BlockMessage::LaneHistoricalRecoveryResponse(response) => {
                            ExactOutputRolloverClaim::HistoricalLaneRecoveryResponse {
                                scope: self.exact_output_scope(),
                                target: peer.clone(),
                                request_hash: response.request_hash,
                                response_hash: HashOf::new(response.as_ref()),
                            }
                        }
                        _ => self.current_lane_output_rollover_claim(message, peer)?,
                    };
                    let wire = BlockMessageWire::try_preencoded(Arc::new(message.clone()))
                        .map_err(|error| error.to_string())?;
                    (
                        vec![NetworkMessage::SumeragiBlock(Arc::new(wire))],
                        vec![peer.clone()],
                        vec![ExactTargetRoute::Topology],
                        None,
                        None,
                        rollover_claim,
                    )
                }
                V2LaneWorkEffect::PostDurableLaneCertificate {
                    peer,
                    reply_routes,
                    ingress_ownership,
                    certificate,
                } => {
                    let reply_routes = reply_routes.as_ref().ok_or_else(|| {
                        "durable lane-certificate response lost its authenticated reply routes"
                            .to_owned()
                    })?;
                    let ingress_ownership = ingress_ownership.as_ref().ok_or_else(|| {
                        "durable lane-certificate response lost its fair-ingress ownership"
                            .to_owned()
                    })?;
                    if !ingress_ownership.validate_exact()
                        || !ingress_ownership.matches_reply_routes(Some(reply_routes))
                    {
                        return Err(
                            "durable lane-certificate response has altered fair-ingress ownership"
                                .to_owned(),
                        );
                    }
                    let (peers, routes, reply_route_history) =
                        Self::exact_target_geometry(peer, Some(reply_routes))?;
                    let wire = BlockMessageWire::try_preencoded(Arc::new(
                        BlockMessage::LaneBlockCertificate(Box::new(certificate.clone())),
                    ))
                    .map_err(|error| error.to_string())?;
                    let descriptor = &certificate.proposal.descriptor;
                    (
                        vec![NetworkMessage::SumeragiBlock(Arc::new(wire))],
                        peers,
                        routes,
                        reply_route_history,
                        Some(ingress_ownership.clone()),
                        ExactOutputRolloverClaim::DurableLaneCertificateResponse {
                            scope: self.exact_output_scope(),
                            target: peer.clone(),
                            lane_id: descriptor.lane_id,
                            lane_block_height: descriptor.lane_block_height,
                            proposal_height: descriptor.proposal_height,
                            proposal_hash: certificate.proposal.proposal_hash,
                            certificate_hash: HashOf::new(certificate),
                        },
                    )
                }
                V2LaneWorkEffect::PostNativeAmx {
                    peer,
                    reply_routes,
                    message,
                } => {
                    let valid = match message {
                        NativeAmxMessage::PrepareRequest(_)
                        | NativeAmxMessage::CommitRequest(_) => reply_routes.is_none(),
                        NativeAmxMessage::PrepareVote(_) | NativeAmxMessage::CommitVote(_) => {
                            reply_routes.is_some()
                        }
                    };
                    if !valid {
                        return Err(
                            "Native AMX effect has invalid reply-route ownership".to_owned()
                        );
                    }
                    let body = native_amx_message_body(message)?;
                    let (peers, routes, reply_route_history) =
                        Self::exact_target_geometry(peer, reply_routes.as_ref())?;
                    (
                        vec![NetworkMessage::NativeAmx(Arc::new(message.clone()))],
                        peers,
                        routes,
                        reply_route_history,
                        None,
                        ExactOutputRolloverClaim::NativeAmx {
                            scope: self.exact_output_scope(),
                            round: body.round,
                            message_hash: HashOf::new(message),
                        },
                    )
                }
                V2LaneWorkEffect::PostLaneDrainVote { peer, vote } => {
                    vote.validate_ingress().map_err(|error| {
                        format!("lane-drain effect has invalid vote evidence: {error}")
                    })?;
                    (
                        vec![NetworkMessage::LaneDrainVote(Box::new(vote.clone()))],
                        vec![peer.clone()],
                        vec![ExactTargetRoute::Topology],
                        None,
                        None,
                        ExactOutputRolloverClaim::LaneDrainVote {
                            scope: self.exact_output_scope(),
                            target: peer.clone(),
                            vote_hash: HashOf::new(vote),
                        },
                    )
                }
                V2LaneWorkEffect::BroadcastMerge(signature) => {
                    let peers = self.remote_voters();
                    let routes = vec![ExactTargetRoute::Topology; peers.len()];
                    (
                        vec![NetworkMessage::MergeCommitteeSignature(Arc::new(
                            signature.clone(),
                        ))],
                        peers,
                        routes,
                        None,
                        None,
                        ExactOutputRolloverClaim::MergeShare {
                            scope: self.exact_output_scope(),
                            share_hash: HashOf::new(signature),
                        },
                    )
                }
                V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
                    peer,
                    view,
                    certificate,
                } => self.queue_plan_effect_parts(
                    peer,
                    *view,
                    certificate,
                    queue_plan_sources.ok_or_else(|| {
                        "QueuePlan admission handoff lacks its Kura batch snapshot".to_owned()
                    })?,
                )?,
                V2LaneWorkEffect::PostCertifiedMergeSidecar {
                    peer,
                    reply_routes,
                    message,
                } => {
                    let valid = match message.as_ref() {
                        CertifiedMergeSidecarMessage::Request(_)
                        | CertifiedMergeSidecarMessage::Close(_) => reply_routes.is_none(),
                        CertifiedMergeSidecarMessage::CloseAck(_)
                        | CertifiedMergeSidecarMessage::GenerationHint(_)
                        | CertifiedMergeSidecarMessage::Chunk(_) => reply_routes.is_some(),
                    };
                    if !valid {
                        return Err(
                            "certified merge-sidecar effect has invalid reply-route ownership"
                                .to_owned(),
                        );
                    }
                    let rollover_claim = match message.as_ref() {
                        CertifiedMergeSidecarMessage::Request(request)
                            if request.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                                && request.requester == self.local_peer
                                && request.responder == *peer =>
                        {
                            ExactOutputRolloverClaim::CertifiedSidecarRequest {
                                scope: self.exact_output_scope(),
                                target: peer.clone(),
                                transfer: CertifiedSidecarTransferIdentity::from_request(request),
                                request_hash: HashOf::new(request),
                            }
                        }
                        CertifiedMergeSidecarMessage::Close(close)
                            if close.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                                && close.closed_through != 0
                                && close.close_id == close.canonical_close_id()
                                && close.requester == self.local_peer
                                && close.responder == *peer =>
                        {
                            ExactOutputRolloverClaim::CertifiedSidecarControl {
                                scope: self.exact_output_scope(),
                                target: peer.clone(),
                                message_hash: HashOf::new(message.as_ref()),
                            }
                        }
                        CertifiedMergeSidecarMessage::CloseAck(ack)
                            if ack.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                                && ack.closed_through != 0
                                && ack.close_id == ack.canonical_close_id()
                                && ack.responder == self.local_peer
                                && ack.requester == *peer =>
                        {
                            ExactOutputRolloverClaim::CertifiedSidecarControl {
                                scope: self.exact_output_scope(),
                                target: peer.clone(),
                                message_hash: HashOf::new(message.as_ref()),
                            }
                        }
                        CertifiedMergeSidecarMessage::GenerationHint(hint)
                            if hint.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                                && hint.hint_id == hint.canonical_hint_id()
                                && hint.responder == self.local_peer
                                && hint.requester == *peer =>
                        {
                            ExactOutputRolloverClaim::CertifiedSidecarControl {
                                scope: self.exact_output_scope(),
                                target: peer.clone(),
                                message_hash: HashOf::new(message.as_ref()),
                            }
                        }
                        CertifiedMergeSidecarMessage::Chunk(chunk)
                            if chunk.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                                && chunk.responder == self.local_peer
                                && chunk.requester == *peer
                                && chunk.chunk_count != 0
                                && chunk.chunk_index < chunk.chunk_count =>
                        {
                            ExactOutputRolloverClaim::CertifiedSidecarChunk {
                                scope: self.exact_output_scope(),
                                target: peer.clone(),
                                transfer: CertifiedSidecarTransferIdentity::from_chunk(chunk),
                                chunk_index: chunk.chunk_index,
                                chunk_count: chunk.chunk_count,
                                response_hash: HashOf::new(chunk),
                            }
                        }
                        _ => {
                            return Err(
                                "certified merge-sidecar effect has no valid rollover claim"
                                    .to_owned(),
                            );
                        }
                    };
                    let (peers, routes, reply_route_history) =
                        Self::exact_target_geometry(peer, reply_routes.as_ref())?;
                    (
                        vec![NetworkMessage::CertifiedMergeSidecar(Arc::clone(message))],
                        peers,
                        routes,
                        reply_route_history,
                        None,
                        rollover_claim,
                    )
                }
            };
        let Some(fanout) = PendingExactFanout::classified_with_route_history(
            messages,
            peers,
            routes,
            reply_route_history,
        )?
        else {
            return Ok(true);
        };
        rollover_claim.validate_fanout(&fanout.messages, &fanout.semantic_peers())?;
        let mut fanout = fanout;
        fanout.ingress_ownership = ingress_ownership;
        fanout.rollover_claim = rollover_claim;
        let pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Err(
                "Sumeragi v2 exact output is sealed after durable finality handoff".to_owned(),
            );
        }
        if fanout
            .targets
            .iter()
            .all(|target| matches!(&target.route, ExactTargetRoute::Reply(_)))
        {
            pending.can_enqueue_owned_reply_transfer(fanout)
        } else {
            pending.can_enqueue(&fanout)
        }
    }
    /// Publish one exact signed body-keeper advert from durable Kura state.
    ///
    /// The advert is rebuilt only after canonical application completes, then
    /// independently revalidated before entering the exact-output corridor.
    /// Its rollover claim remains reconstructible from the same body/finality
    /// source and the frozen height roster.
    fn post_kura_replica_advert_while_guarded(
        &self,
        source: &KuraReplicaAdvertSourceV1,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        let source_height = source.height();
        if source_height == 0 || source_height > self.context.height {
            return Err(
                "Kura replica advert source is outside the active height authority".to_owned(),
            );
        }
        let advert = self
            .kura
            .build_signed_kura_replica_advert_from_source(source, &self.key_pair)
            .map_err(|error| error.to_string())?;
        let rollover_claim = ExactOutputRolloverClaim::DurableKuraReplicaAdvert {
            scope: self.exact_output_scope(),
            source_height,
            advert_hash: HashOf::new(&advert),
        };
        let wire =
            BlockMessageWire::try_preencoded(Arc::new(BlockMessage::KuraReplicaAdvert(advert)))
                .map_err(|error| {
                    format!("failed to encode durable Kura replica advert: {error}")
                })?;
        // The active immutable roster is the only live, bounded transport
        // authority available under validator rotation. Historical departed
        // validators are not guessed or contacted; Kura pins bodies outside
        // the configured proactive horizon fail-closed.
        self.enqueue_exact_fanout_while_guarded(
            vec![NetworkMessage::SumeragiBlock(Arc::new(wire))],
            self.remote_voters(),
            rollover_claim,
            permit,
        )
    }
    fn committee_for_round(&self, round: wire::ConsensusRound) -> Result<Committee, String> {
        if round.context_id != self.context.id() || round.height != self.context.height {
            return Err("Sumeragi v2 committee routing received a foreign round".to_owned());
        }
        Committee::project_indices(
            self.context.height,
            round.view,
            self.context.roster.len(),
            self.context.leader(round.view),
        )
        .map_err(|error| error.to_string())
    }
    fn remote_voters_for_indices(
        &self,
        indices: &[wire::ValidatorIndex],
    ) -> Result<Vec<PeerId>, String> {
        let mut peers = Vec::with_capacity(indices.len());
        for index in indices {
            let roster_index = usize::try_from(*index)
                .map_err(|_| "Sumeragi v2 committee index does not fit usize".to_owned())?;
            let peer = self
                .context
                .roster
                .get(roster_index)
                .ok_or_else(|| "Sumeragi v2 committee index is outside the roster".to_owned())?
                .validator
                .clone();
            if peer != self.local_peer {
                peers.push(peer);
            }
        }
        Ok(peers)
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
    fn deliver_payload_chunk<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
        work_id: EffectWorkId,
        sender: PeerId,
        chunk: wire::PayloadChunk,
        ingress_ownership: FairV2IngressOwnershipEvidence,
    ) -> Result<PayloadChunkDisposition, String> {
        let result = executor.accept_payload_chunk_with_ingress_ownership(
            work_id,
            chunk,
            &sender,
            &ingress_ownership,
            self,
        );
        if let Some(runtime) = ingress_ownership.leader_wire_runtime_receipt() {
            self.leader_wire_ingress
                .mark_leader_wire_volatile_terminal(runtime)?;
        }
        match result {
            Ok(()) => Ok(PayloadChunkDisposition::Delivered),
            Err(EffectTransportError::FailClosed(reason)) => Err(reason),
            Err(error) => {
                iroha_logger::debug!(%sender, %error, "rejected Sumeragi v2 payload chunk");
                Ok(PayloadChunkDisposition::Rejected)
            }
        }
    }
    /// Send one response through every retained authenticated source route.
    pub(crate) fn post_to_peer_on_reply_routes(
        &self,
        peer: PeerId,
        reply_routes: NetworkReplyRoutes,
        ingress_ownership: FairV2IngressOwnershipEvidence,
        message: wire::ConsensusMessageV2,
    ) -> Result<(), String> {
        if reply_routes.semantic_target() != &peer
            || !ingress_ownership.validate_exact()
            || !ingress_ownership.matches_reply_routes(Some(&reply_routes))
        {
            return Err(
                "certified-body response carried altered fair-ingress ownership".to_owned(),
            );
        }
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if reply_routes.is_empty() {
            iroha_logger::debug!(
                "deferred certified Sumeragi v2 response after all retained reply routes retired"
            );
            operation.complete();
            return Ok(());
        }
        let ownership = self.post_block_message_on_reply_routes_while_guarded(
            peer,
            reply_routes,
            ingress_ownership,
            BlockMessage::V2(message),
            operation.permit(),
        )?;
        if ownership == ExactFanoutOwnership::SourceRetained {
            iroha_logger::debug!(
                "deferred certified Sumeragi v2 response to requester reconstruction"
            );
        }
        operation.complete();
        Ok(())
    }
    /// Send one response whose exact payload can be rebuilt from immutable Kura history.
    #[cfg(test)]
    pub(crate) fn post_durable_history_response_with_permit(
        &self,
        peer: PeerId,
        message: wire::ConsensusMessageV2,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<(), String> {
        self.post_durable_history_response_with_routes(peer, None, None, message, permit)
    }
    /// Send a durable historical response through all authenticated source routes.
    pub(crate) fn post_durable_history_response_on_reply_routes_with_permit(
        &self,
        peer: PeerId,
        reply_routes: NetworkReplyRoutes,
        ingress_ownership: FairV2IngressOwnershipEvidence,
        message: wire::ConsensusMessageV2,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<(), String> {
        self.post_durable_history_response_with_routes(
            peer,
            Some(reply_routes),
            Some(ingress_ownership),
            message,
            permit,
        )
    }
    fn post_durable_history_response_with_routes(
        &self,
        peer: PeerId,
        reply_routes: Option<NetworkReplyRoutes>,
        ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
        message: wire::ConsensusMessageV2,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<(), String> {
        match (&reply_routes, &ingress_ownership) {
            (Some(routes), Some(ownership))
                if ownership.validate_exact() && ownership.matches_reply_routes(Some(routes)) => {}
            (None, None) => {}
            (Some(_), Some(_)) => {
                return Err(
                    "durable history response carried altered fair-ingress ownership".to_owned(),
                );
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err("durable history response lost its fair-ingress ownership".to_owned());
            }
        }
        message
            .validate_version()
            .map_err(|error| error.to_string())?;
        let rollover_claim = match &message.payload {
            wire::ConsensusMessageV2Payload::CommitCertificateResponse(response)
                if response.certificate.round.height <= self.context.height
                    && response.responder == self.local_peer =>
            {
                ExactOutputRolloverClaim::DurableCommitCertificateResponse {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    responder: self.local_peer.clone(),
                    source_height: response.certificate.round.height,
                    source_context_id: response.certificate.round.context_id,
                    response_hash: HashOf::new(response),
                }
            }
            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response)
                if response.manifest.round.height <= self.context.height =>
            {
                ExactOutputRolloverClaim::DurableCertifiedBodyResponse {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    responder: self.local_peer.clone(),
                    source_round: response.manifest.round,
                    source_subject: response.manifest.subject,
                    response_hash: HashOf::new(response),
                }
            }
            _ => {
                return Err(
                    "guarded durable-history output is not a non-future Kura response".to_owned(),
                );
            }
        };
        let block_message = Arc::new(BlockMessage::V2(message));
        let wire = BlockMessageWire::try_preencoded(block_message).map_err(|error| {
            format!("failed to encode guarded durable-history response for {peer}: {error}")
        })?;
        let messages = vec![NetworkMessage::SumeragiBlock(Arc::new(wire))];
        let peers = vec![peer];
        rollover_claim.validate_fanout(&messages, &peers)?;
        durable_history_source_covers(
            &messages,
            &rollover_claim,
            &self.context.network_id,
            self.context.height,
            self.kura.as_ref(),
        )?;
        let ownership = match reply_routes {
            Some(reply_routes) => self.enqueue_owned_exact_reply_routes_while_guarded(
                messages
                    .into_iter()
                    .next()
                    .expect("durable response is a singleton"),
                peers
                    .into_iter()
                    .next()
                    .expect("durable response has one target"),
                reply_routes,
                ingress_ownership,
                rollover_claim,
                permit,
            )?,
            None => {
                self.enqueue_exact_fanout_while_guarded(messages, peers, rollover_claim, permit)?
            }
        };
        if ownership == ExactFanoutOwnership::SourceRetained {
            iroha_logger::debug!(
                "deferred historical Sumeragi v2 response to requester reconstruction"
            );
        }
        Ok(())
    }
    /// Send retained lane-local traffic selected by `BlockMessage::is_lane_local`
    /// through the common exact-output corridor.
    pub(crate) fn post_lane_block(
        &self,
        peer: PeerId,
        message: BlockMessage,
    ) -> Result<(), String> {
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if !message.is_lane_local() {
            return Err("v2 lane transport rejected a non-lane block message".to_owned());
        }
        let ownership = self.post_block_message_while_guarded(peer, message, operation.permit())?;
        if ownership == ExactFanoutOwnership::SourceRetained {
            return Err(
                "Sumeragi v2 lane output reached an unreserved corridor boundary".to_owned(),
            );
        }
        operation.complete();
        Ok(())
    }
    /// Send one exact lane certificate reconstructed from its certified Kura artifact.
    #[cfg(test)]
    pub(crate) fn post_durable_lane_certificate(
        &self,
        peer: PeerId,
        certificate: LaneBlockCertificateV1,
    ) -> Result<(), String> {
        self.post_durable_lane_certificate_with_routes(peer, None, None, certificate)
    }
    /// Send a Kura-backed lane certificate through every retained source route.
    pub(crate) fn post_durable_lane_certificate_on_reply_routes(
        &self,
        peer: PeerId,
        reply_routes: NetworkReplyRoutes,
        ingress_ownership: FairV2IngressOwnershipEvidence,
        certificate: LaneBlockCertificateV1,
    ) -> Result<(), String> {
        self.post_durable_lane_certificate_with_routes(
            peer,
            Some(reply_routes),
            Some(ingress_ownership),
            certificate,
        )
    }
    fn post_durable_lane_certificate_with_routes(
        &self,
        peer: PeerId,
        reply_routes: Option<NetworkReplyRoutes>,
        ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
        certificate: LaneBlockCertificateV1,
    ) -> Result<(), String> {
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        match (&reply_routes, &ingress_ownership) {
            (Some(routes), Some(ownership))
                if ownership.validate_exact() && ownership.matches_reply_routes(Some(routes)) => {}
            (None, None) => {}
            (Some(_), Some(_)) => {
                return Err(
                    "durable lane certificate carried altered fair-ingress ownership".to_owned(),
                );
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err("durable lane certificate lost its fair-ingress ownership".to_owned());
            }
        }
        let descriptor = &certificate.proposal.descriptor;
        if descriptor.proposal_height > self.context.height {
            return Err("durable lane certificate belongs to a future global height".to_owned());
        }
        let rollover_claim = ExactOutputRolloverClaim::DurableLaneCertificateResponse {
            scope: self.exact_output_scope(),
            target: peer.clone(),
            lane_id: descriptor.lane_id,
            lane_block_height: descriptor.lane_block_height,
            proposal_height: descriptor.proposal_height,
            proposal_hash: certificate.proposal.proposal_hash,
            certificate_hash: HashOf::new(&certificate),
        };
        let message = Arc::new(BlockMessage::LaneBlockCertificate(Box::new(certificate)));
        let wire = BlockMessageWire::try_preencoded(message).map_err(|error| {
            format!("failed to encode guarded durable lane certificate for {peer}: {error}")
        })?;
        let messages = vec![NetworkMessage::SumeragiBlock(Arc::new(wire))];
        let peers = vec![peer];
        rollover_claim.validate_fanout(&messages, &peers)?;
        durable_history_source_covers(
            &messages,
            &rollover_claim,
            &self.context.network_id,
            self.context.height,
            self.kura.as_ref(),
        )?;
        let ownership = match reply_routes {
            Some(reply_routes) => self.enqueue_owned_exact_reply_routes_while_guarded(
                messages
                    .into_iter()
                    .next()
                    .expect("durable lane response is a singleton"),
                peers
                    .into_iter()
                    .next()
                    .expect("durable lane response has one target"),
                reply_routes,
                ingress_ownership,
                rollover_claim,
                operation.permit(),
            )?,
            None => self.enqueue_exact_fanout_while_guarded(
                messages,
                peers,
                rollover_claim,
                operation.permit(),
            )?,
        };
        if ownership == ExactFanoutOwnership::SourceRetained {
            return Err(
                "durable lane certificate reached an unreserved corridor boundary".to_owned(),
            );
        }
        operation.complete();
        Ok(())
    }
    /// Send one bounded certified merge-sidecar request or response through
    /// the dedicated authenticated network envelope.
    #[cfg(test)]
    pub(crate) fn post_certified_merge_sidecar(
        &self,
        peer: PeerId,
        message: CertifiedMergeSidecarMessage,
    ) {
        let _ = self.post_certified_merge_sidecar_with_reply_routes(peer, None, Arc::new(message));
    }
    /// Send a sidecar request normally or a response on its exact request route.
    pub(crate) fn post_certified_merge_sidecar_with_reply_routes(
        &self,
        peer: PeerId,
        reply_routes: Option<NetworkReplyRoutes>,
        message: Arc<CertifiedMergeSidecarMessage>,
    ) -> Result<ExactFanoutOwnership, String> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        let route_shape_is_valid = match message.as_ref() {
            CertifiedMergeSidecarMessage::Request(_) | CertifiedMergeSidecarMessage::Close(_) => {
                reply_routes.is_none()
            }
            CertifiedMergeSidecarMessage::CloseAck(_)
            | CertifiedMergeSidecarMessage::GenerationHint(_)
            | CertifiedMergeSidecarMessage::Chunk(_) => reply_routes.is_some(),
        };
        if !route_shape_is_valid {
            return Err(
                "certified merge-sidecar request/response has invalid reply-route ownership"
                    .to_owned(),
            );
        }
        let rollover_claim = match message.as_ref() {
            CertifiedMergeSidecarMessage::Request(request)
                if request.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                    && request.requester == self.local_peer
                    && request.responder == peer =>
            {
                ExactOutputRolloverClaim::CertifiedSidecarRequest {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    transfer: CertifiedSidecarTransferIdentity::from_request(request),
                    request_hash: HashOf::new(request),
                }
            }
            CertifiedMergeSidecarMessage::Close(close)
                if close.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                    && close.closed_through != 0
                    && close.close_id == close.canonical_close_id()
                    && close.requester == self.local_peer
                    && close.responder == peer =>
            {
                ExactOutputRolloverClaim::CertifiedSidecarControl {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    message_hash: HashOf::new(message.as_ref()),
                }
            }
            CertifiedMergeSidecarMessage::CloseAck(ack)
                if ack.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                    && ack.closed_through != 0
                    && ack.close_id == ack.canonical_close_id()
                    && ack.responder == self.local_peer
                    && ack.requester == peer =>
            {
                ExactOutputRolloverClaim::CertifiedSidecarControl {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    message_hash: HashOf::new(message.as_ref()),
                }
            }
            CertifiedMergeSidecarMessage::GenerationHint(hint)
                if hint.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                    && hint.hint_id == hint.canonical_hint_id()
                    && hint.responder == self.local_peer
                    && hint.requester == peer =>
            {
                ExactOutputRolloverClaim::CertifiedSidecarControl {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    message_hash: HashOf::new(message.as_ref()),
                }
            }
            CertifiedMergeSidecarMessage::Chunk(chunk)
                if chunk.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                    && chunk.responder == self.local_peer
                    && chunk.requester == peer
                    && chunk.chunk_count != 0
                    && chunk.chunk_index < chunk.chunk_count =>
            {
                ExactOutputRolloverClaim::CertifiedSidecarChunk {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    transfer: CertifiedSidecarTransferIdentity::from_chunk(chunk),
                    chunk_index: chunk.chunk_index,
                    chunk_count: chunk.chunk_count,
                    response_hash: HashOf::new(chunk),
                }
            }
            _ => {
                return Err(
                    "certified merge-sidecar post has no valid semantic rollover claim".to_owned(),
                );
            }
        };
        let data = NetworkMessage::CertifiedMergeSidecar(message);
        let result = match reply_routes {
            Some(reply_routes) => self.enqueue_owned_exact_reply_routes_while_guarded(
                data,
                peer,
                reply_routes,
                None,
                rollover_claim,
                operation.permit(),
            ),
            None => self.enqueue_exact_fanout_while_guarded(
                vec![data],
                vec![peer],
                rollover_claim,
                operation.permit(),
            ),
        };
        let ownership = result?;
        // A concurrent producer can consume the capacity observed by runner
        // preflight. Source retention is bounded backpressure, not loss of the
        // already-owned lane effect, so disarm fail-stop and let the runner
        // return the exact effect to its fair queue.
        operation.complete();
        Ok(ownership)
    }
    /// Send one context-bound Native AMX v2 message to a participant peer.
    #[cfg(test)]
    pub(crate) fn post_native_amx(&self, peer: PeerId, message: NativeAmxMessage) {
        self.post_native_amx_with_reply_routes(peer, None, message);
    }
    /// Send a Native AMX request normally or a request-induced vote on its exact route.
    pub(crate) fn post_native_amx_with_reply_routes(
        &self,
        peer: PeerId,
        reply_routes: Option<NetworkReplyRoutes>,
        message: NativeAmxMessage,
    ) {
        let output_guard = Arc::clone(&self.output_guard);
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            return;
        };
        let route_shape_is_valid = match &message {
            NativeAmxMessage::PrepareRequest(_) | NativeAmxMessage::CommitRequest(_) => {
                reply_routes.is_none()
            }
            NativeAmxMessage::PrepareVote(_) | NativeAmxMessage::CommitVote(_) => {
                reply_routes.is_some()
            }
        };
        if !route_shape_is_valid {
            iroha_logger::error!("Native AMX request/vote has invalid reply-route ownership");
            return;
        }
        let body = match native_amx_message_body(&message) {
            Ok(body)
                if body.round.context_id == self.context.id()
                    && body.round.height == self.context.height =>
            {
                body
            }
            Ok(_) | Err(_) => {
                iroha_logger::error!("Native AMX post has no valid embedded height round");
                return;
            }
        };
        let rollover_claim = ExactOutputRolloverClaim::NativeAmx {
            scope: self.exact_output_scope(),
            round: body.round,
            message_hash: HashOf::new(&message),
        };
        let data = NetworkMessage::NativeAmx(Arc::new(message));
        let result = match reply_routes {
            Some(reply_routes) => self.enqueue_owned_exact_reply_routes_while_guarded(
                data,
                peer,
                reply_routes,
                None,
                rollover_claim,
                operation.permit(),
            ),
            None => self.enqueue_exact_fanout_while_guarded(
                vec![data],
                vec![peer],
                rollover_claim,
                operation.permit(),
            ),
        };
        match result {
            Ok(ExactFanoutOwnership::Owned) => operation.complete(),
            Ok(ExactFanoutOwnership::SourceRetained) => {
                iroha_logger::error!(
                    "Native AMX post reached an unreserved outbound corridor boundary"
                );
            }
            Err(error) => {
                iroha_logger::error!(%error, "Native AMX output failed closed");
            }
        }
    }
    /// Send one exact durably authorized lane-drain vote to a selected peer.
    pub(crate) fn post_lane_drain_vote(&self, peer: PeerId, vote: LaneDrainVoteV1) {
        let output_guard = Arc::clone(&self.output_guard);
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            return;
        };
        if let Err(error) = vote.validate_ingress() {
            iroha_logger::error!(%error, "lane-drain vote output failed validation");
            return;
        }
        let rollover_claim = ExactOutputRolloverClaim::LaneDrainVote {
            scope: self.exact_output_scope(),
            target: peer.clone(),
            vote_hash: HashOf::new(&vote),
        };
        match self.enqueue_exact_fanout_while_guarded(
            vec![NetworkMessage::LaneDrainVote(Box::new(vote))],
            vec![peer],
            rollover_claim,
            operation.permit(),
        ) {
            Ok(ExactFanoutOwnership::Owned) => operation.complete(),
            Ok(ExactFanoutOwnership::SourceRetained) => {
                iroha_logger::error!(
                    "lane-drain vote fanout reached an unreserved outbound corridor boundary"
                );
            }
            Err(error) => {
                iroha_logger::error!(%error, "lane-drain vote output failed closed");
            }
        }
    }
    /// Broadcast one merge signature share to every other frozen voter.
    pub(crate) fn broadcast_merge_to_voters(&self, signature: MergeCommitteeSignature) {
        let output_guard = Arc::clone(&self.output_guard);
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            return;
        };
        let rollover_claim = ExactOutputRolloverClaim::MergeShare {
            scope: self.exact_output_scope(),
            share_hash: HashOf::new(&signature),
        };
        match self.enqueue_exact_fanout_while_guarded(
            vec![NetworkMessage::MergeCommitteeSignature(Arc::new(signature))],
            self.remote_voters(),
            rollover_claim,
            operation.permit(),
        ) {
            Ok(ExactFanoutOwnership::Owned) => operation.complete(),
            Ok(ExactFanoutOwnership::SourceRetained) => {
                iroha_logger::error!(
                    "merge-share fanout reached an unreserved outbound corridor boundary"
                );
            }
            Err(error) => {
                iroha_logger::error!(%error, "merge-share output failed closed");
            }
        }
    }
    fn post_block_message_while_guarded(
        &self,
        peer: PeerId,
        message: BlockMessage,
        _permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        let rollover_claim = match &message {
            BlockMessage::V2(_) => ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
            BlockMessage::LaneHistoricalRecoveryRequest(request) => {
                ExactOutputRolloverClaim::HistoricalLaneRecoveryRequest {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    request_hash: HashOf::new(request.as_ref()),
                }
            }
            BlockMessage::LaneHistoricalRecoveryResponse(response) => {
                ExactOutputRolloverClaim::HistoricalLaneRecoveryResponse {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    request_hash: response.request_hash,
                    response_hash: HashOf::new(response.as_ref()),
                }
            }
            message if message.is_lane_local() => {
                self.current_lane_output_rollover_claim(message, &peer)?
            }
            _ => return Err("guarded v2 output has no typed rollover claim".to_owned()),
        };
        let block_message = Arc::new(message);
        let wire = BlockMessageWire::try_preencoded(block_message).map_err(|error| {
            format!("failed to encode guarded Sumeragi v2 message for {peer}: {error}")
        })?;
        let data = NetworkMessage::SumeragiBlock(Arc::new(wire));
        self.enqueue_exact_fanout_while_guarded(vec![data], vec![peer], rollover_claim, _permit)
    }
    fn post_block_message_on_reply_routes_while_guarded(
        &self,
        peer: PeerId,
        reply_routes: NetworkReplyRoutes,
        ingress_ownership: FairV2IngressOwnershipEvidence,
        message: BlockMessage,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        let rollover_claim = match &message {
            BlockMessage::V2(_) => ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
            BlockMessage::LaneBlockProposal(_)
            | BlockMessage::LaneExecutablePayload(_)
            | BlockMessage::LaneBlockNewViewVote(_)
            | BlockMessage::LaneBlockNewViewCertificate(_)
            | BlockMessage::LaneBlockVote(_)
            | BlockMessage::LaneBlockQc(_)
            | BlockMessage::LaneBlockCertificate(_) => {
                self.current_lane_output_rollover_claim(&message, &peer)?
            }
            _ => return Err("guarded v2 reply has no typed rollover claim".to_owned()),
        };
        let wire = BlockMessageWire::try_preencoded(Arc::new(message)).map_err(|error| {
            format!("failed to encode guarded Sumeragi v2 reply for {peer}: {error}")
        })?;
        self.enqueue_owned_exact_reply_routes_while_guarded(
            NetworkMessage::SumeragiBlock(Arc::new(wire)),
            peer,
            reply_routes,
            Some(ingress_ownership),
            rollover_claim,
            permit,
        )
    }
    fn preencode_v2_network_message(
        message: wire::ConsensusMessageV2,
    ) -> Result<NetworkMessage, String> {
        let wire = BlockMessageWire::try_preencoded(Arc::new(BlockMessage::V2(message)))
            .map_err(|error| format!("failed to encode guarded Sumeragi v2 message: {error}"))?;
        Ok(NetworkMessage::SumeragiBlock(Arc::new(wire)))
    }
    fn broadcast_preencoded_to_voters_while_guarded(
        &self,
        data: &NetworkMessage,
        _permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        self.enqueue_exact_fanout_while_guarded(
            vec![data.clone()],
            self.remote_voters(),
            ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
            _permit,
        )
    }
    /// Broadcast under a caller-owned output permit without reacquiring it.
    pub(crate) fn broadcast_to_voters_while_guarded(
        &self,
        message: wire::ConsensusMessageV2,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<(), String> {
        let data = Self::preencode_v2_network_message(message)?;
        if self.broadcast_preencoded_to_voters_while_guarded(&data, permit)?
            == ExactFanoutOwnership::SourceRetained
        {
            iroha_logger::debug!("deferred block-sync request to its retained discovery source");
        }
        Ok(())
    }
}
include!("v2_worker/current_lane_output_rollover_claim.rs");
impl Drop for ProductionV2Services {
    fn drop(&mut self) {
        let restart_required = !self.clean_teardown;
        if restart_required {
            self.output_guard.close_admission_for_restart();
        }
        self.retire_held_io_completion();
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
include!("v2_worker/effect_services_impl.rs");
/// Unit tests and production-service fixtures shared with the runner tests.
#[cfg(test)]
pub(super) mod tests {
    use norito::codec::Encode as _;

    include!("tests/v2_worker_main_00.rs");
    include!("tests/v2_worker_main_01.rs");
    include!("tests/v2_worker_lifecycle_capacity_cases.rs");
    include!("tests/v2_worker_equivocation_fixture.rs");
    include!("v2_worker/applied_height_handoff_tests.rs");
    include!("v2_worker/queue_plan_admission_handoff_tests.rs");
    include!("v2_worker/upstream_reply_route_test.rs");
    include!("tests/v2_worker_main_02.rs");
    include!("tests/v2_worker_main_04.rs");
    include!("tests/v2_worker_main_05.rs");
}
