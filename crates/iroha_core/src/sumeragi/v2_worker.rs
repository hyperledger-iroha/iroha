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
use super::v2_runtime::RuntimeLifecycleOrdinalSource;
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
        CompletionDisposition, ConsensusBroadcastDisposition, ConsensusSignTask,
        DurableApplyCompletion, EffectExecutorError, EffectExecutorStatus, EffectRuntime,
        EffectTransportError, EffectWorkId, PayloadChunkLifecycleDisposition,
        PendingTipRecoveryAttemptResult, PostFinalityCleanupOutcome, PostFinalityCleanupTarget,
        V2EffectExecutor, V2EffectServices,
    },
    v2_lane_work::{
        DurableLaneRolloverAuthority, V2LaneWorkAdapter, V2LaneWorkEffect,
        durable_historical_lane_output_source_hash, lane_output_identity,
        validate_winning_lane_output,
    },
    v2_lifecycle_coordinator::{
        AuthenticatedSchedulerInputsFactory, CertifiedFetchBodyPersistenceCompletion,
        CertifiedFetchBodyPersistenceId, CertifiedFetchBodyPersistenceTask,
        CertifiedServeTerminalReplayAuthorizationV1, ClaimedCertifiedServeDispatchV1,
        DeferredDurableValidateDispatch, DurableValidateDispatch, ExecutedDurableValidateDispatch,
        LifecycleIngressIoTargetKind, LifecycleIngressIoTargetSeal, LifecycleValidateDispatchKeyV1,
        PreparedLifecycleIngressSelector, PreparedRecoveredDecisionApplyDispatch,
        PreparedRecoveredLifecycleSignDispatch,
        ProductionLifecycleServeRetirementAuthenticationPermitV1,
        ProductionV2CompletionObserverActivationPermitV1, RecoveredDecisionApplyDispatchKeyV1,
        RecoveredDecisionFetchBodyPersistenceCompletionV1,
        RecoveredDecisionFetchBodyPersistenceTaskV1, RecoveredDecisionFetchDispatchKeyV1,
        RecoveredLifecycleSignDispatchIdentityV1, RecoveredLifecycleSignDispatchKeyV1, TurnLease,
    },
    v2_runtime::{LeaderWireRuntimeTerminal, RuntimeQueueLaneSnapshot, SerializedV2Runtime},
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
        let recipient = inbound.sender().clone();
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
            || !ownership.matches_semantic_origin(&recipient)
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
            recipient: sender,
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
/// Lifecycle-owned deterministic validation command.
///
/// Unlike the legacy generic validation task, this command retains the exact
/// registry dispatch and immutable lifecycle address from queue admission
/// through guarded completion publication.
#[must_use = "lifecycle Validate work must remain queue-owned until publication"]
struct LifecycleValidateTaskV1 {
    key: LifecycleValidateDispatchKeyV1,
    dispatch: DurableValidateDispatch,
}
impl LifecycleValidateTaskV1 {
    fn matches_exact(&self) -> bool {
        self.dispatch.matches_dispatch_key(self.key)
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
    LifecycleValidate(LifecycleValidateTaskV1),
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
            | Self::LifecycleValidate(_)
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
            | Self::LifecycleValidate(_)
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
            Self::LifecycleValidate(task) => Some(task.key.lifecycle_ordinal()),
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
            | Self::LifecycleValidate(_)
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
            | Self::LifecycleValidate(_)
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
            | Self::LifecycleValidate(_)
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
            | Self::LifecycleValidate(_)
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
            | Self::LifecycleValidate(_)
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
            | Self::LifecycleValidate(_)
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
    const fn lifecycle_validate_key(&self) -> Option<LifecycleValidateDispatchKeyV1> {
        match self {
            Self::LifecycleValidate(task) => Some(task.key),
            Self::Sign { .. }
            | Self::Store(_)
            | Self::PersistCertifiedFetchBody(_)
            | Self::PersistRecoveredDecisionFetchBody(_)
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
    lifecycle_validate: Option<LifecycleValidateDispatchKeyV1>,
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
        lifecycle_validate: Option<LifecycleValidateDispatchKeyV1>,
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
            lifecycle_validate,
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
    fn transfer_lifecycle_validate_completion_at(
        &self,
        key: LifecycleValidateDispatchKeyV1,
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
            || owner.lifecycle_validate != Some(key)
            || state
                .owned
                .iter()
                .filter(|owned| owned.lifecycle_validate == Some(key))
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
            || owner.lifecycle_validate.is_some()
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
#[derive(Debug)]
struct V2IoTrackedLifecycleValidateV1 {
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
    lifecycle_validates: BTreeMap<LifecycleValidateDispatchKeyV1, V2IoTrackedLifecycleValidateV1>,
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
/// Locked Consensus capacity until one exact lifecycle Validate dispatch enters FIFO.
#[must_use = "the lifecycle Validate reservation must publish its durable dispatch"]
pub(in crate::sumeragi) struct LifecycleValidateCapacityReservationV1<'a> {
    queue: &'a V2IoCommandQueue,
    state: Option<std::sync::MutexGuard<'a, V2IoCommandQueueState>>,
    operation: Option<ConsensusFailStopOperation<'a>>,
    key: LifecycleValidateDispatchKeyV1,
}
impl LifecycleValidateCapacityReservationV1<'_> {
    /// Recheck that registry projection returned the exact attested dispatch.
    pub(in crate::sumeragi) fn preflight(&self, dispatch: &DurableValidateDispatch) -> bool {
        let state = self
            .state
            .as_ref()
            .expect("live lifecycle Validate reservation retains its queue cut");
        dispatch.matches_dispatch_key(self.key)
            && !state.lifecycle_validates.contains_key(&self.key)
    }

    /// Publish the exact durable dispatch after its lifecycle row became Waiting.
    pub(in crate::sumeragi) fn commit(mut self, dispatch: DurableValidateDispatch) {
        assert!(
            self.preflight(&dispatch),
            "reserved lifecycle Validate changed before queue publication"
        );
        let task = LifecycleValidateTaskV1 {
            key: self.key,
            dispatch,
        };
        assert!(task.matches_exact());
        let mut state = self
            .state
            .take()
            .expect("committed lifecycle Validate retains its queue cut");
        let operation = self
            .operation
            .take()
            .expect("committed lifecycle Validate retains its fail-stop operation");
        assert!(
            state
                .lifecycle_validates
                .insert(
                    self.key,
                    V2IoTrackedLifecycleValidateV1 {
                        state: V2IoWorkState::Queued,
                    },
                )
                .is_none(),
            "exact preflight forbids duplicate lifecycle Validate dispatch"
        );
        state
            .commands
            .push_back(V2IoCommand::LifecycleValidate(task));
        drop(state);
        self.queue.ready.notify_all();
        operation.complete();
    }
}
impl Drop for LifecycleValidateCapacityReservationV1<'_> {
    fn drop(&mut self) {
        drop(self.operation.take());
        if let Some(state) = self.state.take() {
            self.queue.admission.release();
            drop(state);
            self.queue.ready.notify_all();
        }
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
    /// One lifecycle-owned Validate bound to its exact registry carrier.
    Validate {
        /// Exact logical Ready ordinal.
        ordinal: u128,
        /// Immutable registry-attested worker key.
        key: LifecycleValidateDispatchKeyV1,
    },
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
    Validate {
        key: LifecycleValidateDispatchKeyV1,
        available: bool,
    },
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
            Self::Validate { available, .. }
            | Self::Apply { available, .. }
            | Self::Sign { available, .. }
            | Self::Fetch { available, .. } => *available,
        }
    }

    const fn predecessor_debt(&self, worker_debt: u64, output_debt: u64) -> u64 {
        match self {
            Self::Validate { .. } | Self::Apply { .. } | Self::Sign { .. } => worker_debt,
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

    /// Transfer the selected Validate row into a locked exact worker reservation.
    pub(in crate::sumeragi) fn select_validate(
        mut self,
        ordinal: u128,
    ) -> Result<LifecycleValidateCapacityReservationV1<'service>, Self> {
        let Some(RecoveredCompletionPreparedCapacityV1::Validate {
            key,
            available: true,
        }) = self.candidates.remove(&ordinal)
        else {
            return Err(self);
        };
        let state = self
            .state
            .take()
            .expect("selected lifecycle Validate retains the worker queue cut");
        let operation = self
            .operation
            .take()
            .expect("selected lifecycle Validate retains the fail-stop operation");
        assert!(
            state.commands.len() < self.queue.capacity
                && self
                    .queue
                    .admission
                    .try_reserve(V2IoAdmissionClass::Consensus),
            "frozen lifecycle Validate capacity changed before selection"
        );
        drop(self.pending.take());
        Ok(LifecycleValidateCapacityReservationV1 {
            queue: self.queue,
            state: Some(state),
            operation: Some(operation),
            key,
        })
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
            lifecycle_validates: BTreeMap::new(),
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
        assert!(
            command.lifecycle_validate_key().is_none(),
            "lifecycle Validate commands require their locked scheduler reservation"
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
                if let Some(key) = command.lifecycle_validate_key() {
                    let tracked = state
                        .lifecycle_validates
                        .get_mut(&key)
                        .expect("queued lifecycle Validate must retain its exact owner");
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
        if let Some(key) = command.lifecycle_validate_key() {
            let tracked = state
                .lifecycle_validates
                .get_mut(&key)
                .expect("queued lifecycle Validate must retain its exact owner");
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
    fn complete_lifecycle_validate(
        &self,
        key: LifecycleValidateDispatchKeyV1,
        result: &ExecutedDurableValidateDispatch,
    ) -> Result<(), String> {
        let mut state = self.lock();
        let tracked = state
            .lifecycle_validates
            .get_mut(&key)
            .ok_or_else(|| "completed lifecycle Validate lost its exact queue owner".to_owned())?;
        if tracked.state != V2IoWorkState::Active || !result.matches_dispatch_key(key) {
            return Err(
                "completed lifecycle Validate changed its exact dispatch material".to_owned(),
            );
        }
        tracked.state = V2IoWorkState::CompletionPending;
        Ok(())
    }
    fn complete_lifecycle_validate_failure(&self, key: LifecycleValidateDispatchKeyV1) {
        let mut state = self.lock();
        let tracked = state
            .lifecycle_validates
            .get_mut(&key)
            .expect("failed lifecycle Validate retains its exact queue owner");
        assert_eq!(tracked.state, V2IoWorkState::Active);
        tracked.state = V2IoWorkState::CompletionPending;
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
    fn transfer_lifecycle_validate_completion(
        self: &Arc<Self>,
        key: LifecycleValidateDispatchKeyV1,
        ownership_position: usize,
    ) -> bool {
        let state = self.lock();
        let pending = state
            .lifecycle_validates
            .get(&key)
            .is_some_and(|tracked| tracked.state == V2IoWorkState::CompletionPending);
        drop(state);
        pending
            && self
                .admission
                .transfer_lifecycle_validate_completion_at(key, ownership_position)
    }
    fn acknowledge_lifecycle_validate(&self, key: LifecycleValidateDispatchKeyV1) {
        let mut state = self.lock();
        let tracked = state
            .lifecycle_validates
            .remove(&key)
            .expect("settled lifecycle Validate retains its exact queue owner");
        assert_eq!(tracked.state, V2IoWorkState::CompletionPending);
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
        assert!(
            state
                .lifecycle_validates
                .values()
                .all(|tracked| tracked.state == V2IoWorkState::CompletionPending),
            "receiver teardown cannot abandon a queued or active lifecycle Validate"
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
            .lifecycle_validates
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
    fn complete_lifecycle_validate(
        &self,
        key: LifecycleValidateDispatchKeyV1,
        result: &ExecutedDurableValidateDispatch,
    ) -> Result<(), String> {
        self.queue.complete_lifecycle_validate(key, result)
    }
    fn complete_lifecycle_validate_failure(&self, key: LifecycleValidateDispatchKeyV1) {
        self.queue.complete_lifecycle_validate_failure(key);
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
include!("v2_worker_completion.rs");
include!("v2_worker_io_execution.rs");
include!("v2_worker_exact_output.rs");
include!("v2_worker_services.rs");
include!("v2_worker_services_impl.rs");
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
