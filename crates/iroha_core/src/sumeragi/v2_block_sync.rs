//! Reducer-preserving sequential block synchronization for Sumeragi v2.
//!
//! This module discovers the CommitQC for the caller's already-frozen active
//! [`wire::HeightContext`] and lets a currently authenticated archive peer
//! serve the exact canonical body after active peers have rolled forward. It never
//! imports a block, writes Kura, forms a certificate, or changes height
//! directly. A successfully authenticated certificate response is converted
//! to the ordinary v2 `QuorumCertificate` envelope and must be admitted through
//! [`super::v2_effects::V2EffectExecutor`]. The sole reducer then persists its
//! decision, requests the body, validates and stores the response, and applies
//! it through the normal WAL path.
//!
//! Responders load self-contained historical finality artifacts from Kura.
//! Each artifact carries the exact frozen context and roster-aligned proofs of
//! possession needed to verify the historical CommitQC, so serving never
//! depends on a second mutable-or-missing copy of the same authority. Their
//! response signature uses their current P2P identity, so validator key
//! rotation does not make a retired height unservable.
#[cfg(test)]
use super::v2::verify_historical_quorum_certificate;
#[cfg(test)]
use super::v2_transport::authenticate_certified_body_request;
use super::v2_transport::{
    AuthenticatedCertifiedBodyRequest, AuthenticatedCommitCertificateResponse,
    OutstandingCommitCertificateRequests, V2TransportError,
    authenticate_certified_body_request_identity,
    authenticate_certified_body_request_with_validator_pops,
    authenticate_commit_certificate_request, authenticate_commit_certificate_request_identity,
};
use super::{
    v2_chunks::encode_payload,
    v2_core::{
        CanonicalIdentityProjection, IDENTITY_DOMAIN_CONTEXT, IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_COMMIT_CERTIFICATE_REQUEST, IDENTITY_KIND_CONSENSUS_MESSAGE,
        IDENTITY_KIND_QUORUM_CERTIFICATE, IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
        ProductionHistoricalCertificateTraceProjection,
        check_production_historical_certificate_transition,
    },
    v2_effects::CommitCertificateReducerAdmission,
};
use crate::kura::Kura;
use core::fmt;
use iroha_crypto::{Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{NetworkId, block::consensus_v2 as wire, peer::PeerId};
use norito::codec::Encode as _;
use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroUsize,
};
use thiserror::Error;
/// One authenticated CommitQC ready for ordinary reducer ingress.
///
/// The request remains outstanding until the caller confirms that
/// [`Self::message`] entered the reducer's serialized queue.
#[derive(Clone, Debug)]
#[must_use]
pub(crate) struct DiscoveredCommitCertificate {
    request_hash: HashOf<wire::CommitCertificateRequest>,
    response: wire::CommitCertificateResponse,
}
impl DiscoveredCommitCertificate {
    /// Build the only consensus input produced by v2 block sync.
    ///
    /// The returned message follows the same authentication, reducer, and WAL
    /// path as a CommitQC received during the live round.
    pub(crate) fn message(&self) -> wire::ConsensusMessageV2 {
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            self.response.certificate.clone(),
        ))
    }
    /// Borrow the authenticated transport response for diagnostics/tests.
    #[cfg(test)]
    pub(crate) const fn response(&self) -> &wire::CommitCertificateResponse {
        &self.response
    }
}
impl From<AuthenticatedCommitCertificateResponse> for DiscoveredCommitCertificate {
    fn from(authenticated: AuthenticatedCommitCertificateResponse) -> Self {
        Self {
            request_hash: authenticated.request_hash(),
            response: authenticated.into_inner(),
        }
    }
}
/// Bounded current-height CommitQC discovery state.
///
/// Construct a fresh instance for each active height. Because a request names
/// exactly this immutable context and the caller acknowledges it only after
/// reducer admission, dropping the instance at height rollover safely rejects
/// all delayed responses from the previous height.
pub(crate) struct V2BlockSyncDiscovery {
    context: wire::HeightContext,
    requester: PeerId,
    outstanding: OutstandingCommitCertificateRequests,
}
/// Chain-scoped bounded server state for historical CommitQC discovery.
///
/// Exact retransmissions reuse the signed cached response without another disk
/// read. Request signature bytes are deliberately excluded from the cache
/// identity: after the transport boundary authenticates a signature variant
/// over the same immutable request, the server rebinds the already-validated
/// cached response to its new exact request hash instead of waiting for
/// unrelated FIFO eviction. A request that changes any unsigned field still
/// conflicts with the occupied logical slot. A serving-key rotation uses the
/// same re-signing path so the response identity always matches the current
/// authenticated outer peer. Historical body responses are bounded by both
/// entry count and aggregate canonical wire bytes; a response larger than the
/// cache byte ceiling is still served, but is not retained.
pub(crate) struct V2BlockSyncServer {
    network_id: NetworkId,
    capacity: usize,
    responses: BTreeMap<HashOf<wire::CommitCertificateRequest>, wire::ConsensusMessageV2>,
    identities: BTreeMap<CommitCertificateServerIdentity, CachedCommitCertificateRequestIdentity>,
    order: VecDeque<HashOf<wire::CommitCertificateRequest>>,
    body_responses: BTreeMap<HashOf<wire::CertifiedBodyRequest>, CachedHistoricalBodyResponse>,
    body_identities: BTreeMap<HistoricalBodyRequestIdentity, CachedHistoricalBodyRequestIdentity>,
    body_order: VecDeque<HashOf<wire::CertifiedBodyRequest>>,
    body_response_byte_capacity: usize,
    body_response_bytes: usize,
}
impl V2BlockSyncServer {
    /// Construct an empty bounded server for one exact network identity.
    pub(crate) fn new(network_id: NetworkId, capacity: usize) -> Result<Self, V2BlockSyncError> {
        // Bound persistent history-response retention independently of the
        // ingress byte queues while leaving oversized responses serviceable.
        let body_response_byte_capacity = usize::try_from(wire::MAX_DA_ENCODED_PAYLOAD_BYTES)?;
        Self::new_with_body_response_byte_capacity(
            network_id,
            capacity,
            body_response_byte_capacity,
        )
    }
    fn new_with_body_response_byte_capacity(
        network_id: NetworkId,
        capacity: usize,
        body_response_byte_capacity: usize,
    ) -> Result<Self, V2BlockSyncError> {
        if capacity == 0 {
            return Err(V2TransportError::ZeroCapacity.into());
        }
        Ok(Self {
            network_id,
            capacity,
            responses: BTreeMap::new(),
            identities: BTreeMap::new(),
            order: VecDeque::new(),
            body_responses: BTreeMap::new(),
            body_identities: BTreeMap::new(),
            body_order: VecDeque::new(),
            body_response_byte_capacity,
            body_response_bytes: 0,
        })
    }
    /// Authenticate and answer one exact request from canonical Kura history.
    pub(crate) fn serve(
        &mut self,
        kura: &Kura,
        request: wire::CommitCertificateRequest,
        authenticated_requester: &PeerId,
        responder_key: &KeyPair,
    ) -> Result<Option<wire::ConsensusMessageV2>, V2BlockSyncError> {
        self.serve_with(request, authenticated_requester, responder_key, |request| {
            serve_commit_certificate_from_kura(
                kura,
                request.clone(),
                authenticated_requester,
                responder_key,
            )
        })
    }
    /// Serve an exact historical canonical body when this authenticated node's
    /// durable history contains the applied block and matching finality
    /// artifact.
    ///
    /// Kura's self-contained finality artifact supplies the historical context,
    /// roster proofs, and exact certified subject. The archive peer need not
    /// have signed the historical QC: the QC authenticates the subject, and the
    /// peer signs the response containing the exact subject-bound bytes. The
    /// receiver still stores and validates the returned body through its active
    /// reducer effects; this service never imports or applies a block locally.
    pub(crate) fn serve_historical_body(
        &mut self,
        kura: &Kura,
        request: wire::CertifiedBodyRequest,
        authenticated_requester: &PeerId,
        responder_key: &KeyPair,
    ) -> Result<Option<wire::ConsensusMessageV2>, V2BlockSyncError> {
        self.serve_historical_body_with(
            request,
            authenticated_requester,
            responder_key,
            |request| {
                build_historical_body_response(
                    kura,
                    request.clone(),
                    authenticated_requester,
                    responder_key,
                )
            },
        )
    }
    fn serve_historical_body_with<Build>(
        &mut self,
        request: wire::CertifiedBodyRequest,
        authenticated_requester: &PeerId,
        responder_key: &KeyPair,
        build: Build,
    ) -> Result<Option<wire::ConsensusMessageV2>, V2BlockSyncError>
    where
        Build: FnOnce(
            &wire::CertifiedBodyRequest,
        ) -> Result<Option<wire::ConsensusMessageV2>, V2BlockSyncError>,
    {
        authenticate_certified_body_request_identity(&request, authenticated_requester)?;
        self.serve_authenticated_historical_body_with(request, responder_key, build)
    }
    fn serve_authenticated_historical_body_with<Build>(
        &mut self,
        request: wire::CertifiedBodyRequest,
        responder_key: &KeyPair,
        build: Build,
    ) -> Result<Option<wire::ConsensusMessageV2>, V2BlockSyncError>
    where
        Build: FnOnce(
            &wire::CertifiedBodyRequest,
        ) -> Result<Option<wire::ConsensusMessageV2>, V2BlockSyncError>,
    {
        let request_hash = HashOf::new(&request);
        let unsigned_request_hash = Hash::new(&request.signature_preimage());
        let responder = PeerId::new(responder_key.public_key().clone());
        let identity = HistoricalBodyRequestIdentity::from(&request);
        if let Some(cached) = self.body_responses.get(&request_hash) {
            if cached.responder == responder {
                return Ok(Some(cached.message.clone()));
            }
        }
        if let Some(existing) = self.body_identities.get(&identity).copied() {
            if existing.unsigned_request_hash != unsigned_request_hash {
                return Err(V2BlockSyncError::ConflictingHistoricalBodyRequest {
                    existing: existing.request_hash,
                    incoming: request_hash,
                });
            }
            let cached = self
                .body_responses
                .get(&existing.request_hash)
                .ok_or(V2BlockSyncError::CorruptServerCache)?;
            let response = rebind_cached_historical_body_response(
                &cached.message,
                request_hash,
                responder.clone(),
                responder_key,
            )?;
            self.remove_body(existing.request_hash)?;
            return self.retain_historical_body_response(
                request_hash,
                identity,
                unsigned_request_hash,
                responder,
                response,
            );
        }
        if self.body_responses.contains_key(&request_hash) {
            return Err(V2BlockSyncError::CorruptServerCache);
        }
        let Some(response) = build(&request)? else {
            return Ok(None);
        };
        self.retain_historical_body_response(
            request_hash,
            identity,
            unsigned_request_hash,
            responder,
            response,
        )
    }
    fn retain_historical_body_response(
        &mut self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
        identity: HistoricalBodyRequestIdentity,
        unsigned_request_hash: Hash,
        responder: PeerId,
        response: wire::ConsensusMessageV2,
    ) -> Result<Option<wire::ConsensusMessageV2>, V2BlockSyncError> {
        let response_bytes = response.encoded_len();
        if response_bytes > self.body_response_byte_capacity {
            return Ok(Some(response));
        }
        while self.body_responses.len() >= self.capacity
            || self
                .body_response_bytes
                .checked_add(response_bytes)
                .is_none_or(|retained| retained > self.body_response_byte_capacity)
        {
            let Some(oldest) = self.body_order.pop_front() else {
                return Err(V2BlockSyncError::CorruptServerCache);
            };
            self.remove_body(oldest)?;
        }
        let retained_bytes = self
            .body_response_bytes
            .checked_add(response_bytes)
            .ok_or(V2BlockSyncError::CorruptServerCache)?;
        self.body_responses.insert(
            request_hash,
            CachedHistoricalBodyResponse {
                responder,
                message: response.clone(),
                retained_bytes: response_bytes,
            },
        );
        self.body_identities.insert(
            identity,
            CachedHistoricalBodyRequestIdentity {
                request_hash,
                unsigned_request_hash,
            },
        );
        self.body_order.push_back(request_hash);
        self.body_response_bytes = retained_bytes;
        Ok(Some(response))
    }
    fn serve_with<Build>(
        &mut self,
        request: wire::CommitCertificateRequest,
        authenticated_requester: &PeerId,
        responder_key: &KeyPair,
        build: Build,
    ) -> Result<Option<wire::ConsensusMessageV2>, V2BlockSyncError>
    where
        Build: FnOnce(
            &wire::CommitCertificateRequest,
        ) -> Result<Option<wire::ConsensusMessageV2>, V2BlockSyncError>,
    {
        authenticate_commit_certificate_request_identity(&request, authenticated_requester)?;
        self.serve_authenticated_with(request, responder_key, build)
    }
    fn serve_authenticated_with<Build>(
        &mut self,
        request: wire::CommitCertificateRequest,
        responder_key: &KeyPair,
        build: Build,
    ) -> Result<Option<wire::ConsensusMessageV2>, V2BlockSyncError>
    where
        Build: FnOnce(
            &wire::CommitCertificateRequest,
        ) -> Result<Option<wire::ConsensusMessageV2>, V2BlockSyncError>,
    {
        if request.network_id != self.network_id || request.height == 0 {
            return Err(wire::ValidationError::WrongHeightContext.into());
        }
        let request_hash = HashOf::new(&request);
        let unsigned_request_hash = Hash::new(&request.signature_preimage());
        let responder = PeerId::new(responder_key.public_key().clone());
        let identity = CommitCertificateServerIdentity::from(&request);
        if let Some(cached) = self.responses.get(&request_hash) {
            let current_identity = matches!(
                &cached.payload,
                wire::ConsensusMessageV2Payload::CommitCertificateResponse(response)
                    if response.responder == responder
            );
            if current_identity {
                return Ok(Some(cached.clone()));
            }
        }
        if let Some(existing) = self.identities.get(&identity).copied() {
            if existing.unsigned_request_hash != unsigned_request_hash {
                return Err(V2BlockSyncError::ConflictingServerRequest {
                    existing: existing.request_hash,
                    incoming: request_hash,
                });
            }
            let cached = self
                .responses
                .get(&existing.request_hash)
                .ok_or(V2BlockSyncError::CorruptServerCache)?;
            let response =
                rebind_cached_commit_certificate_response(cached, request_hash, responder_key)?;
            self.remove(existing.request_hash);
            self.retain_commit_certificate_response(
                request_hash,
                identity,
                unsigned_request_hash,
                response.clone(),
            )?;
            return Ok(Some(response));
        }
        if self.responses.contains_key(&request_hash) {
            return Err(V2BlockSyncError::CorruptServerCache);
        }
        let Some(response) = build(&request)? else {
            return Ok(None);
        };
        self.retain_commit_certificate_response(
            request_hash,
            identity,
            unsigned_request_hash,
            response.clone(),
        )?;
        Ok(Some(response))
    }
    fn retain_commit_certificate_response(
        &mut self,
        request_hash: HashOf<wire::CommitCertificateRequest>,
        identity: CommitCertificateServerIdentity,
        unsigned_request_hash: Hash,
        response: wire::ConsensusMessageV2,
    ) -> Result<(), V2BlockSyncError> {
        while self.responses.len() >= self.capacity {
            let Some(oldest) = self.order.pop_front() else {
                return Err(V2BlockSyncError::CorruptServerCache);
            };
            self.remove(oldest);
        }
        self.responses.insert(request_hash, response);
        self.identities.insert(
            identity,
            CachedCommitCertificateRequestIdentity {
                request_hash,
                unsigned_request_hash,
            },
        );
        self.order.push_back(request_hash);
        Ok(())
    }
    fn remove(&mut self, request_hash: HashOf<wire::CommitCertificateRequest>) {
        if self.responses.remove(&request_hash).is_none() {
            return;
        }
        self.identities
            .retain(|_, cached| cached.request_hash != request_hash);
        self.order.retain(|hash| *hash != request_hash);
    }
    fn remove_body(
        &mut self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
    ) -> Result<(), V2BlockSyncError> {
        let Some(cached) = self.body_responses.get(&request_hash) else {
            return Ok(());
        };
        let remaining_bytes = self
            .body_response_bytes
            .checked_sub(cached.retained_bytes)
            .ok_or(V2BlockSyncError::CorruptServerCache)?;
        self.body_responses.remove(&request_hash);
        self.body_identities
            .retain(|_, cached| cached.request_hash != request_hash);
        self.body_order.retain(|hash| *hash != request_hash);
        self.body_response_bytes = remaining_bytes;
        Ok(())
    }
    #[cfg(test)]
    fn len(&self) -> usize {
        self.responses.len()
    }
    #[cfg(test)]
    fn body_len(&self) -> usize {
        self.body_responses.len()
    }
    #[cfg(test)]
    fn body_response_bytes(&self) -> usize {
        self.body_response_bytes
    }
}
#[derive(Clone, Debug)]
struct CachedHistoricalBodyResponse {
    responder: PeerId,
    message: wire::ConsensusMessageV2,
    retained_bytes: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CachedHistoricalBodyRequestIdentity {
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    unsigned_request_hash: Hash,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CachedCommitCertificateRequestIdentity {
    request_hash: HashOf<wire::CommitCertificateRequest>,
    unsigned_request_hash: Hash,
}
fn rebind_cached_historical_body_response(
    cached: &wire::ConsensusMessageV2,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    responder: PeerId,
    responder_key: &KeyPair,
) -> Result<wire::ConsensusMessageV2, V2BlockSyncError> {
    let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(mut response) =
        cached.payload.clone()
    else {
        return Err(V2BlockSyncError::CorruptServerCache);
    };
    response.request_hash = request_hash;
    response.responder = responder;
    response.signature.clear();
    response.signature =
        Signature::new(responder_key.private_key(), &response.signature_preimage())
            .payload()
            .to_vec();
    Ok(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
    ))
}
fn rebind_cached_commit_certificate_response(
    cached: &wire::ConsensusMessageV2,
    request_hash: HashOf<wire::CommitCertificateRequest>,
    responder_key: &KeyPair,
) -> Result<wire::ConsensusMessageV2, V2BlockSyncError> {
    let wire::ConsensusMessageV2Payload::CommitCertificateResponse(mut response) =
        cached.payload.clone()
    else {
        return Err(V2BlockSyncError::CorruptServerCache);
    };
    response.request_hash = request_hash;
    response.responder = PeerId::new(responder_key.public_key().clone());
    response.signature.clear();
    response.signature =
        Signature::new(responder_key.private_key(), &response.signature_preimage())
            .payload()
            .to_vec();
    Ok(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(response),
    ))
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct HistoricalBodyRequestIdentity {
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    requester: PeerId,
}
impl From<&wire::CertifiedBodyRequest> for HistoricalBodyRequestIdentity {
    fn from(request: &wire::CertifiedBodyRequest) -> Self {
        Self {
            round: request.round,
            subject: request.subject,
            requester: request.requester.clone(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CommitCertificateServerIdentity {
    protocol_version: u16,
    context_id: wire::HeightContextId,
    height: wire::Height,
    requester: PeerId,
}
impl From<&wire::CommitCertificateRequest> for CommitCertificateServerIdentity {
    fn from(request: &wire::CommitCertificateRequest) -> Self {
        Self {
            protocol_version: request.protocol_version,
            context_id: request.context_id,
            height: request.height,
            requester: request.requester.clone(),
        }
    }
}
impl V2BlockSyncDiscovery {
    /// Construct discovery state for one validated active context.
    pub(crate) fn new(
        context: wire::HeightContext,
        requester: PeerId,
        max_outstanding: usize,
    ) -> Result<Self, V2BlockSyncError> {
        context.validate()?;
        Ok(Self {
            context,
            requester,
            outstanding: OutstandingCommitCertificateRequests::new(max_outstanding)?,
        })
    }
    /// Sign and register the exact current-height request.
    ///
    /// A duplicate call is rejected rather than replacing outstanding state;
    /// use [`Self::retransmit`] to resend the same signed bytes.
    pub(crate) fn begin(
        &mut self,
        requester_key: &KeyPair,
    ) -> Result<wire::ConsensusMessageV2, V2BlockSyncError> {
        ensure_key_identity(requester_key, &self.requester)?;
        let mut request = wire::CommitCertificateRequest {
            protocol_version: wire::PROTOCOL_VERSION,
            network_id: self.context.network_id,
            context_id: self.context.id(),
            height: self.context.height,
            requester: self.requester.clone(),
            signature: Vec::new(),
        };
        request.signature =
            Signature::new(requester_key.private_key(), &request.signature_preimage())
                .payload()
                .to_vec();
        let authenticated = authenticate_commit_certificate_request(
            &self.context,
            request.clone(),
            &self.requester,
        )?;
        self.outstanding.register(authenticated)?;
        Ok(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CommitCertificateRequest(request),
        ))
    }
    /// Rebuild the exact registered envelope for deterministic retransmission.
    pub(crate) fn retransmit(
        &self,
        request_hash: HashOf<wire::CommitCertificateRequest>,
    ) -> Option<wire::ConsensusMessageV2> {
        self.outstanding
            .request(request_hash)
            .cloned()
            .map(|request| {
                wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::CommitCertificateRequest(request),
                )
            })
    }
    /// Authenticate a response without consuming its outstanding request.
    ///
    /// The caller next submits [`DiscoveredCommitCertificate::message`] via
    /// ordinary consensus ingress, where the production aggregate verifier
    /// authenticates the CommitQC under the frozen roster, and invokes
    /// [`Self::complete`] only when that enqueue succeeds.
    pub(crate) fn authenticate_response(
        &self,
        response: wire::CommitCertificateResponse,
        authenticated_responder: &PeerId,
    ) -> Result<DiscoveredCommitCertificate, V2BlockSyncError> {
        self.outstanding
            .authenticate_response(&self.context, response, authenticated_responder)
            .map(Into::into)
            .map_err(Into::into)
    }
    /// Acknowledge one response after its CommitQC entered reducer ingress.
    pub(crate) fn complete(&mut self, discovered: DiscoveredCommitCertificate) -> bool {
        self.outstanding.complete(discovered.request_hash)
    }
    /// Cancel one request when ordinary consensus reaches Decision before its
    /// discovery response is needed.
    pub(crate) fn cancel(&mut self, request_hash: HashOf<wire::CommitCertificateRequest>) -> bool {
        self.outstanding.complete(request_hash)
    }
    /// Enqueue the ordinary CommitQC message and atomically retire discovery
    /// state only after the enqueue callback succeeds.
    ///
    /// This is the preferred production handoff. Runtime backpressure leaves
    /// the exact request outstanding, allowing the same authenticated response
    /// or a retransmitted request to make progress later.
    pub(crate) fn enqueue_and_complete<Enqueue, EnqueueError>(
        &mut self,
        discovered: DiscoveredCommitCertificate,
        enqueue: Enqueue,
    ) -> Result<(), CommitCertificateAdmissionError<EnqueueError>>
    where
        Enqueue: FnOnce(
            wire::ConsensusMessageV2,
        ) -> Result<CommitCertificateReducerAdmission, EnqueueError>,
    {
        let message = discovered.message();
        let request_hash = discovered.request_hash;
        let response_request_hash = discovered.response.request_hash;
        let certificate = discovered.response.certificate.clone();
        let request_present_before = self.outstanding.contains(request_hash);
        let admitted_message_hash = historical_typed_identity(
            IDENTITY_DOMAIN_PAYLOAD,
            IDENTITY_KIND_CONSENSUS_MESSAGE,
            HashOf::new(&message),
        );
        let historical_trace = ProductionHistoricalCertificateTraceProjection {
            context_id: historical_typed_identity(
                IDENTITY_DOMAIN_CONTEXT,
                IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
                self.context.id().0,
            ),
            context_height: self.context.height,
            certificate_context_id: historical_typed_identity(
                IDENTITY_DOMAIN_CONTEXT,
                IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
                certificate.round.context_id.0,
            ),
            certificate_height: certificate.round.height,
            request_hash: historical_typed_identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_COMMIT_CERTIFICATE_REQUEST,
                request_hash,
            ),
            response_request_hash: historical_typed_identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_COMMIT_CERTIFICATE_REQUEST,
                response_request_hash,
            ),
            response_certificate: historical_typed_identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_QUORUM_CERTIFICATE,
                HashOf::new(&certificate),
            ),
            message_certificate: match &message.payload {
                wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
                    historical_typed_identity(
                        IDENTITY_DOMAIN_PAYLOAD,
                        IDENTITY_KIND_QUORUM_CERTIFICATE,
                        HashOf::new(certificate),
                    )
                }
                _ => CanonicalIdentityProjection::zero(),
            },
            message_hash: admitted_message_hash,
            admitted_message_hash,
            request_present_before,
            request_present_after: false,
        };
        let Some(checked_transition) =
            check_production_historical_certificate_transition(historical_trace)
        else {
            return Err(CommitCertificateAdmissionError::RefinementRejected);
        };
        let historical_trace = checked_transition.into_projection();
        let admission =
            enqueue(message.clone()).map_err(CommitCertificateAdmissionError::Enqueue)?;
        if !admission.matches(&message)
            || admission.refinement_projection() != historical_trace.admitted_message_hash
        {
            return Err(CommitCertificateAdmissionError::MismatchedReducerAdmission);
        }
        if !self.complete(discovered) {
            return Err(CommitCertificateAdmissionError::RequestDisappeared);
        }
        if self.outstanding.contains(request_hash) {
            return Err(CommitCertificateAdmissionError::RefinementRejected);
        }
        Ok(())
    }
    /// Number of bounded outstanding requests.
    #[cfg(test)]
    pub(crate) fn outstanding_len(&self) -> usize {
        self.outstanding.len()
    }
}
fn historical_typed_identity<T>(
    domain: u8,
    kind: u8,
    hash: HashOf<T>,
) -> CanonicalIdentityProjection {
    CanonicalIdentityProjection::from_bytes(domain, kind, *hash.as_ref())
}
/// Serve a signed request from a canonical historical Kura finality artifact.
///
/// Identity authentication happens before disk access. A missing artifact is
/// reported as `Ok(None)` so peers can query another validator; corruption or
/// a canonical-block mismatch propagates fail-closed from Kura.
fn serve_commit_certificate_from_kura(
    kura: &Kura,
    request: wire::CommitCertificateRequest,
    authenticated_requester: &PeerId,
    responder_key: &KeyPair,
) -> Result<Option<wire::ConsensusMessageV2>, V2BlockSyncError> {
    authenticate_commit_certificate_request_identity(&request, authenticated_requester)?;
    if request.height == 0 {
        return Err(wire::ValidationError::WrongHeightContext.into());
    }
    let Some(artifact) = kura.v2_finality_artifact(request.height)? else {
        return Ok(None);
    };
    serve_commit_certificate_from_artifact(
        &artifact,
        request,
        authenticated_requester,
        responder_key,
    )
    .map(Some)
}
/// Build a response from an already-loaded, immutable finality artifact.
///
/// This factored form supports deterministic adversarial tests. Production
/// serving uses [`serve_commit_certificate_from_kura`] so the artifact is also
/// checked against the canonical durable block hash.
fn serve_commit_certificate_from_artifact(
    artifact: &wire::finality::V2FinalityArtifact,
    request: wire::CommitCertificateRequest,
    authenticated_requester: &PeerId,
    responder_key: &KeyPair,
) -> Result<wire::ConsensusMessageV2, V2BlockSyncError> {
    artifact.validate()?;
    let authenticated = authenticate_commit_certificate_request(
        &artifact.height_context,
        request,
        authenticated_requester,
    )?;
    let responder = PeerId::new(responder_key.public_key().clone());
    let mut response = wire::CommitCertificateResponse {
        request_hash: authenticated.request_hash(),
        certificate: artifact.commit_qc.clone(),
        responder,
        signature: Vec::new(),
    };
    response.signature =
        Signature::new(responder_key.private_key(), &response.signature_preimage())
            .payload()
            .to_vec();
    Ok(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(response),
    ))
}
fn build_historical_body_response(
    kura: &Kura,
    request: wire::CertifiedBodyRequest,
    authenticated_requester: &PeerId,
    responder_key: &KeyPair,
) -> Result<Option<wire::ConsensusMessageV2>, V2BlockSyncError> {
    let height = request.round.height;
    let Some(artifact) = kura.v2_finality_artifact(height)? else {
        return Ok(None);
    };
    let context = &artifact.height_context;
    let proofs_of_possession = &artifact.validator_set_pops;
    let authenticated: AuthenticatedCertifiedBodyRequest =
        authenticate_certified_body_request_with_validator_pops(
            context,
            proofs_of_possession,
            request,
            authenticated_requester,
        )?;
    let request = authenticated.request();
    if request.subject != artifact.subject {
        return Err(V2BlockSyncError::HistoricalSubjectMismatch { height });
    }
    let responder = PeerId::new(responder_key.public_key().clone());
    let block_height = usize::try_from(height)?;
    let block_height = NonZeroUsize::new(block_height)
        .ok_or(V2BlockSyncError::MissingHistoricalBlock { height })?;
    let block = kura
        .get_block(block_height)
        .ok_or(V2BlockSyncError::MissingHistoricalBlock { height })?;
    if block.hash() != request.subject.block_hash {
        return Err(V2BlockSyncError::HistoricalSubjectMismatch { height });
    }
    // Kura retains the canonical result-bearing execution image. Consensus
    // body transport must project it back to the exact resultless proposal
    // authenticated by the QC subject.
    let proposal = block.canonical_resultless_proposal();
    let body = proposal
        .encode_wire()
        .map_err(|error| V2BlockSyncError::CanonicalBody(error.to_string()))?;
    if !proposal.is_resultless_proposal() || Hash::new(&body) != request.subject.payload_hash {
        return Err(V2BlockSyncError::HistoricalSubjectMismatch { height });
    }
    // Once the authenticated QC subject is proven to be the exact finalized
    // Kura block and proposal payload, either QC phase is sufficient body-
    // availability evidence. Keep this exhaustive so any future phase must be
    // considered explicitly before it can use historical serving.
    match request.certificate.phase {
        wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit => {}
    }
    let encoded = encode_payload(context, request.round, request.subject, &body)?;
    let (manifest, _) = encoded.into_parts();
    let mut response = wire::CertifiedBodyResponse {
        request_hash: authenticated.request_hash(),
        manifest,
        body,
        responder: responder.clone(),
        signature: Vec::new(),
    };
    response.signature =
        Signature::new(responder_key.private_key(), &response.signature_preimage())
            .payload()
            .to_vec();
    response.validate_against(context, request, &responder)?;
    Ok(Some(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
    )))
}
fn ensure_key_identity(key: &KeyPair, peer: &PeerId) -> Result<(), V2BlockSyncError> {
    if key.public_key() != peer.public_key() {
        return Err(V2BlockSyncError::LocalKeyMismatch);
    }
    Ok(())
}
/// Failures at the v2-only sequential sync boundary.
#[derive(Debug, Error)]
pub(crate) enum V2BlockSyncError {
    /// A prior fatal consensus operation requires process restart.
    #[error("Sumeragi v2 block synchronization requires process restart")]
    RestartRequired,
    /// A locally built historical response could not cross canonical transport encoding.
    #[error("failed to post a guarded Sumeragi v2 block-sync response: {0}")]
    ResponsePost(String),
    /// A wire value does not match the requested historical context.
    #[error(transparent)]
    Wire(#[from] wire::ValidationError),
    /// A transport identity, signature, request binding, or certificate failed.
    #[error(transparent)]
    Transport(#[from] V2TransportError),
    /// A persisted finality artifact is malformed or internally inconsistent.
    #[error(transparent)]
    Finality(#[from] wire::finality::V2FinalityValidationError),
    /// Kura could not read or validate its immutable canonical finality sidecar.
    #[error(transparent)]
    Kura(#[from] crate::kura::Error),
    /// Canonical body chunking under the frozen DA layout failed.
    #[error(transparent)]
    Chunk(#[from] super::v2_chunks::V2ChunkError),
    /// A bounded height/index conversion overflowed local representation.
    #[error(transparent)]
    Integer(#[from] std::num::TryFromIntError),
    /// The supplied signing key is not the configured local P2P identity.
    #[error("Sumeragi v2 block-sync signing key differs from the configured local peer")]
    LocalKeyMismatch,
    /// A request changed authenticated unsigned fields within one cached logical slot.
    #[error("Sumeragi v2 block-sync request {incoming} conflicts with cached request {existing}")]
    ConflictingServerRequest {
        /// Hash of the already cached exact request.
        existing: HashOf<wire::CommitCertificateRequest>,
        /// Hash of the conflicting request.
        incoming: HashOf<wire::CommitCertificateRequest>,
    },
    /// Internal bounded response-cache indexes diverged.
    #[error("Sumeragi v2 block-sync response cache is internally inconsistent")]
    CorruptServerCache,
    /// The requested CommitQC subject differs from Kura's canonical decision.
    #[error("Sumeragi v2 historical certified subject differs at height {height}")]
    HistoricalSubjectMismatch {
        /// Conflicting historical height.
        height: wire::Height,
    },
    /// Kura finality exists without its canonical block body.
    #[error("Sumeragi v2 historical canonical block is missing at height {height}")]
    MissingHistoricalBlock {
        /// Missing historical height.
        height: wire::Height,
    },
    /// Canonical `SignedBlockWire` encoding failed.
    #[error("failed to encode historical canonical body: {0}")]
    CanonicalBody(String),
    /// A body request changed authenticated unsigned fields within one cached logical slot.
    #[error("Sumeragi v2 body request {incoming} conflicts with cached request {existing}")]
    ConflictingHistoricalBodyRequest {
        /// Hash of the cached exact request.
        existing: HashOf<wire::CertifiedBodyRequest>,
        /// Hash of the conflicting request.
        incoming: HashOf<wire::CertifiedBodyRequest>,
    },
}
/// Failure while handing an authenticated discovered CommitQC to reducer ingress.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CommitCertificateAdmissionError<E> {
    /// The serialized reducer queue rejected or deferred the message.
    Enqueue(E),
    /// The callback returned reducer ownership for a different canonical message.
    MismatchedReducerAdmission,
    /// The exact request disappeared between authentication and serialized enqueue.
    RequestDisappeared,
    /// The authenticated discovery handoff failed its shared pure refinement gate.
    RefinementRejected,
}
impl<E: fmt::Display> fmt::Display for CommitCertificateAdmissionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Enqueue(error) => write!(formatter, "CommitQC reducer enqueue failed: {error}"),
            Self::MismatchedReducerAdmission => formatter
                .write_str("CommitQC reducer admission belongs to a different canonical message"),
            Self::RequestDisappeared => formatter.write_str(
                "CommitQC discovery request disappeared before reducer admission completed",
            ),
            Self::RefinementRejected => formatter
                .write_str("CommitQC discovery failed its exact historical refinement gate"),
        }
    }
}
impl<E> std::error::Error for CommitCertificateAdmissionError<E> where E: std::error::Error + 'static
{}
#[cfg(test)]
/// Shared deterministic fixtures for sibling Sumeragi v2 unit tests.
pub(super) mod tests {
    use super::*;
    use crate::{block::ValidBlock, sumeragi::v2_transport::OutstandingCertifiedBodyRequests};
    use iroha_crypto::{Algorithm, Hash};
    use iroha_data_model::{NetworkId, block::BlockHeader};
    use std::{cell::Cell, num::NonZeroU64, sync::Arc};
    fn test_network_id(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([seed; Hash::LENGTH]),
        ))
    }
    struct Fixture {
        context: wire::HeightContext,
        old_validators: Vec<KeyPair>,
        proofs_of_possession: Vec<Vec<u8>>,
        requester: KeyPair,
        rotated_responder: KeyPair,
        artifact: wire::finality::V2FinalityArtifact,
    }
    impl Fixture {
        fn new() -> Self {
            let mut old_validators = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                        .expect("deterministic historical validator key")
                })
                .collect::<Vec<_>>();
            old_validators.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let roster = old_validators
                .iter()
                .map(|key| wire::ValidatorPower {
                    validator: peer(key),
                    power: 1,
                })
                .collect::<Vec<_>>();
            let context = wire::HeightContext {
                network_id: test_network_id(0x81),
                protocol_version: wire::PROTOCOL_VERSION,
                height: 1,
                epoch: 0,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum: wire::DualQuorum::from_roster(&roster).expect("equal-vote quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"v2 sync nexus/amx context"),
                execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 1024,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 4096,
                    max_chunk_count: 8,
                },
                leader_seed: [0x71; 32],
            };
            let subject = wire::BlockSubject {
                parent_block_hash: None,
                block_hash: block_hash(0x41),
                payload_hash: Hash::new(b"historical canonical body"),
            };
            let mut commit_qc = wire::QuorumCertificate {
                round: wire::ConsensusRound {
                    context_id: context.id(),
                    height: context.height,
                    view: 7,
                },
                proposal_round: wire::ConsensusRound {
                    context_id: context.id(),
                    height: context.height,
                    view: 7,
                },
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment: execution_commitment(0x41),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xC1; 96],
            };
            commit_qc.aggregate_signature = aggregate_certificate(&commit_qc, &old_validators);
            let proofs_of_possession = old_validators
                .iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("BLS proof of possession")
                })
                .collect::<Vec<_>>();
            let artifact = wire::finality::V2FinalityArtifact::new(
                context.clone(),
                subject,
                commit_qc,
                proofs_of_possession.clone(),
            );
            artifact.validate().expect("valid historical artifact");
            Self {
                context,
                old_validators,
                proofs_of_possession,
                requester: key(90),
                rotated_responder: key(91),
                artifact,
            }
        }
        fn discovery(&self) -> V2BlockSyncDiscovery {
            V2BlockSyncDiscovery::new(self.context.clone(), peer(&self.requester), 1)
                .expect("valid discovery")
        }
        fn signed_request(
            &self,
            discovery: &mut V2BlockSyncDiscovery,
        ) -> wire::CommitCertificateRequest {
            let envelope = discovery.begin(&self.requester).expect("begin request");
            let wire::ConsensusMessageV2Payload::CommitCertificateRequest(request) =
                envelope.payload
            else {
                panic!("begin emits only commit-certificate request")
            };
            request
        }
        fn response(
            &self,
            request: wire::CommitCertificateRequest,
        ) -> wire::CommitCertificateResponse {
            let envelope = serve_commit_certificate_from_artifact(
                &self.artifact,
                request,
                &peer(&self.requester),
                &self.rotated_responder,
            )
            .expect("serve historical artifact");
            let wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) =
                envelope.payload
            else {
                panic!("serve emits only commit-certificate response")
            };
            response
        }
        fn body_request(&self, certificate: wire::QuorumCertificate) -> wire::CertifiedBodyRequest {
            let mut request = wire::CertifiedBodyRequest {
                round: certificate.proposal_round,
                subject: certificate.subject,
                certificate,
                requester: peer(&self.requester),
                signature: Vec::new(),
            };
            request.signature =
                Signature::new(self.requester.private_key(), &request.signature_preimage())
                    .payload()
                    .to_vec();
            request
        }
    }
    fn key(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).expect("deterministic BLS key")
    }
    fn peer(key: &KeyPair) -> PeerId {
        PeerId::new(key.public_key().clone())
    }
    fn block_hash(seed: u8) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::prehashed([seed; Hash::LENGTH]))
    }
    fn execution_commitment(seed: u8) -> wire::ExecutionCommitment {
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new([seed, 1]),
            Hash::new([seed, 2]),
            Hash::new([seed, 3]),
            1,
            Hash::new([seed, 4]),
        )
    }
    fn aggregate_certificate(certificate: &wire::QuorumCertificate, keys: &[KeyPair]) -> Vec<u8> {
        let preimage = wire::Vote {
            round: certificate.round,
            proposal_round: certificate.proposal_round,
            phase: certificate.phase,
            subject: certificate.subject,
            execution_commitment: certificate.execution_commitment,
            signer: certificate.signers[0],
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = certificate
            .signers
            .iter()
            .map(|index| {
                Signature::new(
                    keys[usize::try_from(*index).expect("small signer index")].private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
        iroha_crypto::bls_normal_aggregate_signatures(&refs).expect("aggregate QC")
    }
    fn resign_request(request: &mut wire::CommitCertificateRequest, key: &KeyPair) {
        request.signature = Signature::new(key.private_key(), &request.signature_preimage())
            .payload()
            .to_vec();
    }
    fn resign_response(response: &mut wire::CommitCertificateResponse, key: &KeyPair) {
        response.signature = Signature::new(key.private_key(), &response.signature_preimage())
            .payload()
            .to_vec();
    }
    fn body_cache_response(
        fixture: &Fixture,
        request: &wire::CertifiedBodyRequest,
    ) -> wire::ConsensusMessageV2 {
        let body = b"historical canonical body".to_vec();
        assert_eq!(Hash::new(&body), request.subject.payload_hash);
        let encoded = encode_payload(&fixture.context, request.round, request.subject, &body)
            .expect("encode cache-test body");
        let (manifest, _) = encoded.into_parts();
        let mut response = wire::CertifiedBodyResponse {
            request_hash: HashOf::new(request),
            manifest,
            body,
            responder: peer(&fixture.old_validators[0]),
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            fixture.old_validators[0].private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::CertifiedBodyResponse(
            response,
        ))
    }
    /// Exact historical source and responses shared with worker rollover tests.
    pub(in crate::sumeragi) struct DurableHistoryFixture {
        /// Kura containing the canonical block and finality artifact.
        pub(in crate::sumeragi) kura: Arc<Kura>,
        /// Historical finality artifact at height one.
        pub(in crate::sumeragi) artifact: wire::finality::V2FinalityArtifact,
        /// Historical validator keys used only by deterministic tests.
        pub(in crate::sumeragi) validators: Vec<KeyPair>,
        /// Authenticated requester targeted by both responses.
        pub(in crate::sumeragi) requester: PeerId,
        /// Signed CommitQC response reconstructed from the finality artifact.
        pub(in crate::sumeragi) commit_response: wire::ConsensusMessageV2,
        /// Signed body response reconstructed from the canonical block.
        pub(in crate::sumeragi) body_response: wire::ConsensusMessageV2,
    }
    /// Build exact Kura-backed historical responses for worker rollover tests.
    pub(in crate::sumeragi) fn durable_history_fixture() -> DurableHistoryFixture {
        let fixture = Fixture::new();
        let committed = ValidBlock::new_dummy_and_modify_header(
            fixture.old_validators[0].private_key(),
            |header| {
                header.set_height(NonZeroU64::new(1).expect("non-zero height"));
                header.set_prev_block_hash(None);
                header.set_view_change_index(4);
                header.merkle_root = None;
            },
        )
        .commit_unchecked()
        .unpack(|_| {});
        let mut executed_block: iroha_data_model::block::SignedBlock = committed.into();
        executed_block
            .set_transaction_results(Vec::new(), &[], Vec::new())
            .expect("attach deterministic history-fixture results");
        let executed_block_wire = executed_block
            .encode_wire()
            .expect("encode executed history-fixture block");
        let executed_block_wire_len =
            u64::try_from(executed_block_wire.len()).expect("executed wire length fits u64");
        let executed_block_wire_hash = Hash::new(&executed_block_wire);
        let proposal = executed_block.canonical_resultless_proposal();
        let canonical_wire = proposal
            .encode_wire()
            .expect("encode history-fixture proposal");
        let block = Arc::new(executed_block);
        let mut context = fixture.context.clone();
        context.da_layout = wire::SumeragiV2GenesisContextParameters::recommended().da_layout;
        context.validate().expect("valid history-fixture context");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let mut execution = execution_commitment(0x43);
        execution.executed_block_wire_len = executed_block_wire_len;
        execution.executed_block_wire_hash = executed_block_wire_hash;
        let mut certificate = wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 4,
            },
            proposal_round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 4,
            },
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: execution,
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        certificate.aggregate_signature =
            aggregate_certificate(&certificate, &fixture.old_validators);
        let artifact = wire::finality::V2FinalityArtifact::new(
            context.clone(),
            subject,
            certificate.clone(),
            fixture.proofs_of_possession.clone(),
        );
        artifact.validate().expect("valid history-fixture finality");
        let kura = Kura::blank_kura_for_testing();
        kura.store_block(block)
            .expect("store history-fixture block");
        let _receipt = kura
            .store_v2_finality_artifact(&artifact)
            .expect("store history-fixture finality");
        let requester = peer(&fixture.requester);
        let mut commit_request = wire::CommitCertificateRequest {
            protocol_version: wire::PROTOCOL_VERSION,
            network_id: context.network_id,
            context_id: context.id(),
            height: context.height,
            requester: requester.clone(),
            signature: Vec::new(),
        };
        resign_request(&mut commit_request, &fixture.requester);
        let commit_response = serve_commit_certificate_from_artifact(
            &artifact,
            commit_request,
            &requester,
            &fixture.old_validators[3],
        )
        .expect("build durable CommitQC response");
        let body_request = fixture.body_request(certificate);
        let body_response = build_historical_body_response(
            kura.as_ref(),
            body_request,
            &requester,
            &fixture.old_validators[3],
        )
        .expect("build durable body response")
        .expect("history fixture has a frozen-roster archive responder");
        DurableHistoryFixture {
            kura,
            artifact,
            validators: fixture.old_validators,
            requester,
            commit_response,
            body_response,
        }
    }
    #[test]
    fn discovery_outputs_only_normal_commit_qc_ingress_and_waits_for_enqueue() {
        let fixture = Fixture::new();
        let mut discovery = fixture.discovery();
        let request = fixture.signed_request(&mut discovery);
        let request_hash = HashOf::new(&request);
        assert_eq!(discovery.outstanding_len(), 1);
        assert_eq!(
            discovery.retransmit(request_hash),
            Some(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CommitCertificateRequest(request.clone())
            ))
        );
        let response = fixture.response(request.clone());
        let late_replay = response.clone();
        let discovered = discovery
            .authenticate_response(response, &peer(&fixture.rotated_responder))
            .expect("authenticate response");
        assert_eq!(discovery.outstanding_len(), 1);
        assert!(matches!(
            discovered.message().payload,
            wire::ConsensusMessageV2Payload::QuorumCertificate(ref certificate)
                if certificate == &fixture.artifact.commit_qc
        ));
        let rejected = discovery.enqueue_and_complete(discovered.clone(), |_| {
            Err::<CommitCertificateReducerAdmission, _>("runtime backpressure")
        });
        assert_eq!(
            rejected,
            Err(CommitCertificateAdmissionError::Enqueue(
                "runtime backpressure"
            ))
        );
        assert_eq!(discovery.outstanding_len(), 1);
        let foreign_admission =
            CommitCertificateReducerAdmission::for_test(&wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CommitCertificateRequest(request.clone()),
            ));
        let mismatched = discovery.enqueue_and_complete(discovered.clone(), |_| {
            Ok::<_, &'static str>(foreign_admission)
        });
        assert_eq!(
            mismatched,
            Err(CommitCertificateAdmissionError::MismatchedReducerAdmission)
        );
        assert_eq!(
            discovery.outstanding_len(),
            1,
            "foreign reducer ownership must not retire authenticated discovery"
        );
        let mut enqueued = None;
        discovery
            .enqueue_and_complete(discovered, |message| {
                let admission = CommitCertificateReducerAdmission::for_test(&message);
                enqueued = Some(message);
                Ok::<_, &'static str>(admission)
            })
            .expect("successful reducer enqueue retires request");
        assert_eq!(discovery.outstanding_len(), 0);
        assert!(matches!(
            enqueued.expect("captured enqueue").payload,
            wire::ConsensusMessageV2Payload::QuorumCertificate(_)
        ));
        assert!(matches!(
            discovery.authenticate_response(late_replay, &peer(&fixture.rotated_responder)),
            Err(V2BlockSyncError::Transport(
                V2TransportError::UnsolicitedCommitCertificateResponse(_)
            ))
        ));
    }
    #[test]
    fn historical_artifact_is_served_by_rotated_current_identity() {
        let fixture = Fixture::new();
        assert!(
            fixture
                .old_validators
                .iter()
                .all(|key| key.public_key() != fixture.rotated_responder.public_key())
        );
        let mut discovery = fixture.discovery();
        let request = fixture.signed_request(&mut discovery);
        let response = fixture.response(request);
        assert_eq!(response.responder, peer(&fixture.rotated_responder));
        let _ = discovery
            .authenticate_response(response, &peer(&fixture.rotated_responder))
            .expect("current identity can serve a historical QC");
    }
    #[test]
    fn cross_chain_context_and_spoofed_requests_are_rejected() {
        let fixture = Fixture::new();
        let mut discovery = fixture.discovery();
        let request = fixture.signed_request(&mut discovery);
        let mut cross_chain = request.clone();
        cross_chain.network_id = test_network_id(0x82);
        resign_request(&mut cross_chain, &fixture.requester);
        assert!(matches!(
            serve_commit_certificate_from_artifact(
                &fixture.artifact,
                cross_chain,
                &peer(&fixture.requester),
                &fixture.rotated_responder,
            ),
            Err(V2BlockSyncError::Transport(V2TransportError::Wire(
                wire::ValidationError::WrongHeightContext
            )))
        ));
        let spoof = peer(&fixture.old_validators[0]);
        assert!(matches!(
            serve_commit_certificate_from_artifact(
                &fixture.artifact,
                request.clone(),
                &spoof,
                &fixture.rotated_responder,
            ),
            Err(V2BlockSyncError::Transport(
                V2TransportError::OuterIdentityMismatch {
                    kind: super::super::v2_transport::TransportIdentityKind::CommitCertificateRequester,
                    ..
                }
            ))
        ));
        let mut wrong_context = request;
        wrong_context.context_id =
            wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(b"wrong context")));
        resign_request(&mut wrong_context, &fixture.requester);
        assert!(
            serve_commit_certificate_from_artifact(
                &fixture.artifact,
                wrong_context,
                &peer(&fixture.requester),
                &fixture.rotated_responder,
            )
            .is_err()
        );
    }
    #[test]
    fn unsolicited_spoofed_and_invalid_qc_responses_leave_request_live() {
        let fixture = Fixture::new();
        let mut discovery = fixture.discovery();
        let request = fixture.signed_request(&mut discovery);
        let request_hash = HashOf::new(&request);
        let valid = fixture.response(request);
        let mut unsolicited = valid.clone();
        unsolicited.request_hash = HashOf::from_untyped_unchecked(Hash::new(b"unsolicited"));
        resign_response(&mut unsolicited, &fixture.rotated_responder);
        assert!(matches!(
            discovery.authenticate_response(unsolicited, &peer(&fixture.rotated_responder)),
            Err(V2BlockSyncError::Transport(
                V2TransportError::UnsolicitedCommitCertificateResponse(_)
            ))
        ));
        assert!(discovery.outstanding.contains(request_hash));
        let spoof = peer(&fixture.old_validators[0]);
        assert!(matches!(
            discovery.authenticate_response(valid.clone(), &spoof),
            Err(V2BlockSyncError::Transport(
                V2TransportError::OuterIdentityMismatch {
                    kind: super::super::v2_transport::TransportIdentityKind::CommitCertificateResponder,
                    ..
                }
            ))
        ));
        assert!(discovery.outstanding.contains(request_hash));
        let invalid_aggregate = discovery
            .authenticate_response(valid.clone(), &peer(&fixture.rotated_responder))
            .expect("transport authenticates before ordinary QC ingress");
        assert_eq!(
            discovery.enqueue_and_complete(invalid_aggregate, |_| {
                Err::<CommitCertificateReducerAdmission, _>("invalid aggregate")
            }),
            Err(CommitCertificateAdmissionError::Enqueue(
                "invalid aggregate"
            ))
        );
        assert!(discovery.outstanding.contains(request_hash));
        let mut prepare = valid;
        prepare.certificate.phase = wire::GlobalPhase::Prepare;
        resign_response(&mut prepare, &fixture.rotated_responder);
        assert!(
            discovery
                .authenticate_response(prepare, &peer(&fixture.rotated_responder))
                .is_err()
        );
        assert!(discovery.outstanding.contains(request_hash));
    }
    #[test]
    fn catch_up_is_strictly_sequential_across_contexts() {
        let fixture = Fixture::new();
        let mut height_one = fixture.discovery();
        let request_one = fixture.signed_request(&mut height_one);
        let response_one = fixture.response(request_one);
        let discovered_one = height_one
            .authenticate_response(response_one.clone(), &peer(&fixture.rotated_responder))
            .expect("height-one response");
        height_one
            .enqueue_and_complete(discovered_one, |message| {
                Ok::<_, &'static str>(CommitCertificateReducerAdmission::for_test(&message))
            })
            .expect("height one reducer admission");
        let mut context_two = fixture.context.clone();
        context_two.height = 2;
        context_two.parent_commit_qc = Some(fixture.artifact.commit_qc.clone());
        context_two.validate().expect("successor context");
        let subject_two = wire::BlockSubject {
            parent_block_hash: Some(fixture.artifact.subject.block_hash),
            block_hash: block_hash(0x42),
            payload_hash: Hash::new(b"height two canonical body"),
        };
        let commit_two = wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: context_two.id(),
                height: 2,
                view: 1,
            },
            proposal_round: wire::ConsensusRound {
                context_id: context_two.id(),
                height: 2,
                view: 1,
            },
            phase: wire::GlobalPhase::Commit,
            subject: subject_two,
            execution_commitment: execution_commitment(0x42),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xC2; 96],
        };
        let artifact_two = wire::finality::V2FinalityArtifact::new(
            context_two.clone(),
            subject_two,
            commit_two,
            fixture.proofs_of_possession.clone(),
        );
        artifact_two.validate().expect("height-two artifact");
        let mut height_two = V2BlockSyncDiscovery::new(context_two, peer(&fixture.requester), 1)
            .expect("height-two discovery");
        let request_two = fixture.signed_request(&mut height_two);
        assert!(matches!(
            height_two.authenticate_response(response_one, &peer(&fixture.rotated_responder)),
            Err(V2BlockSyncError::Transport(
                V2TransportError::UnsolicitedCommitCertificateResponse(_)
            ))
        ));
        let response_two = serve_commit_certificate_from_artifact(
            &artifact_two,
            request_two,
            &peer(&fixture.requester),
            &fixture.rotated_responder,
        )
        .expect("serve height two");
        let wire::ConsensusMessageV2Payload::CommitCertificateResponse(response_two) =
            response_two.payload
        else {
            panic!("commit response")
        };
        let discovered_two = height_two
            .authenticate_response(response_two, &peer(&fixture.rotated_responder))
            .expect("exact successor response");
        assert_eq!(
            discovered_two.response().certificate,
            artifact_two.commit_qc
        );
    }
    #[test]
    fn local_key_mismatch_and_duplicate_request_fail_without_replacement() {
        let fixture = Fixture::new();
        let mut discovery = fixture.discovery();
        assert!(matches!(
            discovery.begin(&fixture.rotated_responder),
            Err(V2BlockSyncError::LocalKeyMismatch)
        ));
        let request = fixture.signed_request(&mut discovery);
        let request_hash = HashOf::new(&request);
        assert!(matches!(
            discovery.begin(&fixture.requester),
            Err(V2BlockSyncError::Transport(
                V2TransportError::DuplicateCommitCertificateRequest(hash)
            )) if hash == request_hash
        ));
        assert_eq!(discovery.outstanding_len(), 1);
    }
    #[test]
    fn server_cache_is_bounded_replay_safe_and_key_rotation_aware() {
        let fixture = Fixture::new();
        let mut discovery = fixture.discovery();
        let request = fixture.signed_request(&mut discovery);
        let request_hash = HashOf::new(&request);
        let mut server =
            V2BlockSyncServer::new(fixture.context.network_id.clone(), 1).expect("server");
        let builds = Cell::new(0_u32);
        let first = server
            .serve_with(
                request.clone(),
                &peer(&fixture.requester),
                &fixture.rotated_responder,
                |request| {
                    builds.set(builds.get() + 1);
                    serve_commit_certificate_from_artifact(
                        &fixture.artifact,
                        request.clone(),
                        &peer(&fixture.requester),
                        &fixture.rotated_responder,
                    )
                    .map(Some)
                },
            )
            .expect("first response")
            .expect("artifact exists");
        assert_eq!(builds.get(), 1);
        assert_eq!(server.len(), 1);
        let replay = server
            .serve_with(
                request.clone(),
                &peer(&fixture.requester),
                &fixture.rotated_responder,
                |_| -> Result<_, V2BlockSyncError> { panic!("exact replay must not reload Kura") },
            )
            .expect("cached replay")
            .expect("cached response");
        assert_eq!(replay, first);
        assert_eq!(builds.get(), 1);
        let new_responder = key(92);
        let rotated = server
            .serve_with(
                request,
                &peer(&fixture.requester),
                &new_responder,
                |_| -> Result<_, V2BlockSyncError> {
                    panic!("serving-key rotation must not reload Kura")
                },
            )
            .expect("re-sign after key rotation")
            .expect("rotated response");
        assert_eq!(builds.get(), 1);
        assert_ne!(rotated, first);
        let wire::ConsensusMessageV2Payload::CommitCertificateResponse(rotated) = rotated.payload
        else {
            panic!("response payload")
        };
        assert_eq!(rotated.responder, peer(&new_responder));
        assert_eq!(rotated.request_hash, request_hash);
        Signature::try_from_bytes(&rotated.signature)
            .expect("parse key-rotated response signature")
            .verify(new_responder.public_key(), &rotated.signature_preimage())
            .expect("verify key-rotated response signature");
        assert_eq!(server.identities.len(), 1);
        assert_eq!(server.order, VecDeque::from([request_hash]));
        let second_requester = key(93);
        let mut second_discovery =
            V2BlockSyncDiscovery::new(fixture.context.clone(), peer(&second_requester), 1)
                .expect("second discovery");
        let second = second_discovery
            .begin(&second_requester)
            .expect("second request");
        let wire::ConsensusMessageV2Payload::CommitCertificateRequest(second) = second.payload
        else {
            panic!("request payload")
        };
        server
            .serve_with(
                second,
                &peer(&second_requester),
                &new_responder,
                |request| {
                    serve_commit_certificate_from_artifact(
                        &fixture.artifact,
                        request.clone(),
                        &peer(&second_requester),
                        &new_responder,
                    )
                    .map(Some)
                },
            )
            .expect("bounded cache evicts old response")
            .expect("second response");
        assert_eq!(server.len(), 1);
    }
    #[test]
    fn server_rebinds_authenticated_signature_variant_without_waiting_for_eviction() {
        let fixture = Fixture::new();
        let requester = &fixture.requester;
        let requester_peer = peer(requester);
        let mut discovery = fixture.discovery();
        let first = discovery.begin(requester).expect("first signed request");
        let wire::ConsensusMessageV2Payload::CommitCertificateRequest(first) = first.payload else {
            panic!("discovery emits a certificate request")
        };
        let mut restarted = first.clone();
        restarted.signature[0] ^= 0x01;
        assert_eq!(first.signature_preimage(), restarted.signature_preimage());
        assert_ne!(first.signature, restarted.signature);
        let first_hash = HashOf::new(&first);
        let restarted_hash = HashOf::new(&restarted);
        assert_ne!(first_hash, restarted_hash);

        let responder = &fixture.rotated_responder;
        let mut server =
            V2BlockSyncServer::new(fixture.context.network_id.clone(), 1).expect("server");
        server
            .serve_with(first, &requester_peer, responder, |request| {
                serve_commit_certificate_from_artifact(
                    &fixture.artifact,
                    request.clone(),
                    &requester_peer,
                    responder,
                )
                .map(Some)
            })
            .expect("serve first signature")
            .expect("artifact exists");
        let rebound = server
            // Enter below the independently tested transport authenticator to
            // isolate cache behavior for a distinct authenticated signature
            // encoding over the same unsigned request.
            .serve_authenticated_with(
                restarted.clone(),
                responder,
                |_| -> Result<_, V2BlockSyncError> {
                    panic!("same unsigned request must reuse cached history")
                },
            )
            .expect("rebind restarted request")
            .expect("cached response exists");
        let wire::ConsensusMessageV2Payload::CommitCertificateResponse(rebound) = rebound.payload
        else {
            panic!("server emits a certificate response")
        };
        assert_eq!(rebound.request_hash, restarted_hash);
        assert_eq!(rebound.certificate, fixture.artifact.commit_qc);
        rebound
            .validate_against(&fixture.context, &restarted)
            .expect("rebound response names the exact signature variant");
        Signature::try_from_bytes(&rebound.signature)
            .expect("parse rebound response signature")
            .verify(responder.public_key(), &rebound.signature_preimage())
            .expect("verify rebound response signature");
        assert_eq!(server.len(), 1);
        assert!(!server.responses.contains_key(&first_hash));
        assert!(server.responses.contains_key(&restarted_hash));
        assert_eq!(server.identities.len(), 1);
        assert_eq!(server.order, VecDeque::from([restarted_hash]));
    }
    #[test]
    fn server_returns_none_for_missing_canonical_artifact_without_caching() {
        let fixture = Fixture::new();
        let mut discovery = fixture.discovery();
        let request = fixture.signed_request(&mut discovery);
        let kura = Kura::blank_kura_for_testing();
        let mut server =
            V2BlockSyncServer::new(fixture.context.network_id.clone(), 2).expect("server");
        assert!(
            server
                .serve(
                    kura.as_ref(),
                    request,
                    &peer(&fixture.requester),
                    &fixture.rotated_responder,
                )
                .expect("missing artifact is not corruption")
                .is_none()
        );
        assert_eq!(server.len(), 0);
    }
    #[test]
    fn server_rejects_spoof_and_cross_chain_request_before_history_lookup() {
        let fixture = Fixture::new();
        let mut discovery = fixture.discovery();
        let request = fixture.signed_request(&mut discovery);
        let mut server =
            V2BlockSyncServer::new(fixture.context.network_id.clone(), 2).expect("server");
        let spoof = peer(&fixture.old_validators[0]);
        assert!(matches!(
            server.serve_with(
                request.clone(),
                &spoof,
                &fixture.rotated_responder,
                |_| -> Result<_, V2BlockSyncError> {
                    panic!("spoofed request must not read history")
                },
            ),
            Err(V2BlockSyncError::Transport(
                V2TransportError::OuterIdentityMismatch { .. }
            ))
        ));
        let mut cross_chain = request;
        cross_chain.network_id = test_network_id(0x83);
        resign_request(&mut cross_chain, &fixture.requester);
        assert!(matches!(
            server.serve_with(
                cross_chain,
                &peer(&fixture.requester),
                &fixture.rotated_responder,
                |_| -> Result<_, V2BlockSyncError> {
                    panic!("cross-chain request must not read history")
                },
            ),
            Err(V2BlockSyncError::Wire(
                wire::ValidationError::WrongHeightContext
            ))
        ));
        assert_eq!(server.len(), 0);
    }
    #[test]
    fn historical_body_server_rejects_spoof_before_history_lookup() {
        let fixture = Fixture::new();
        let request = fixture.body_request(fixture.artifact.commit_qc.clone());
        let mut server =
            V2BlockSyncServer::new(fixture.context.network_id.clone(), 2).expect("server");
        assert!(matches!(
            server.serve_historical_body_with(
                request,
                &peer(&fixture.old_validators[3]),
                &fixture.old_validators[0],
                |_| -> Result<_, V2BlockSyncError> {
                    panic!("spoofed body request must not read history")
                },
            ),
            Err(V2BlockSyncError::Transport(
                V2TransportError::OuterIdentityMismatch { .. }
            ))
        ));
        assert_eq!(server.body_len(), 0);
    }
    #[test]
    fn historical_body_cache_rebinds_authenticated_signature_variant() {
        let fixture = Fixture::new();
        let requester = &fixture.requester;
        let requester_peer = peer(requester);
        let first = fixture.body_request(fixture.artifact.commit_qc.clone());
        let mut restarted = first.clone();
        restarted.signature[0] ^= 0x01;
        assert_eq!(first.signature_preimage(), restarted.signature_preimage());
        assert_ne!(first.signature, restarted.signature);
        let first_hash = HashOf::new(&first);
        let restarted_hash = HashOf::new(&restarted);
        assert_ne!(first_hash, restarted_hash);

        let responder = &fixture.old_validators[0];
        let first_response = body_cache_response(&fixture, &first);
        let mut server =
            V2BlockSyncServer::new(fixture.context.network_id.clone(), 1).expect("server");
        server
            .serve_historical_body_with(first, &requester_peer, responder, |_| {
                Ok(Some(first_response))
            })
            .expect("serve first signature")
            .expect("body exists");
        let rebound = server
            // Enter below the independently tested transport authenticator to
            // isolate cache behavior for a distinct authenticated signature
            // encoding over the same unsigned request.
            .serve_authenticated_historical_body_with(
                restarted.clone(),
                responder,
                |_| -> Result<_, V2BlockSyncError> {
                    panic!("same unsigned body request must reuse cached history")
                },
            )
            .expect("rebind restarted body request")
            .expect("cached body exists");
        let rebound_bytes = rebound.encoded_len();
        let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(rebound) = rebound.payload
        else {
            panic!("server emits a body response")
        };
        assert_eq!(rebound.request_hash, restarted_hash);
        Signature::try_from_bytes(&rebound.signature)
            .expect("parse rebound body signature")
            .verify(responder.public_key(), &rebound.signature_preimage())
            .expect("verify rebound body signature");
        assert_eq!(server.body_len(), 1);
        assert_eq!(server.body_response_bytes(), rebound_bytes);
        assert!(!server.body_responses.contains_key(&first_hash));
        assert!(server.body_responses.contains_key(&restarted_hash));
        assert_eq!(server.body_identities.len(), 1);
        assert_eq!(server.body_order, VecDeque::from([restarted_hash]));

        let mut changed_unsigned = restarted;
        changed_unsigned.certificate.aggregate_signature[0] ^= 0x01;
        let changed_hash = HashOf::new(&changed_unsigned);
        assert!(matches!(
            server.serve_authenticated_historical_body_with(
                changed_unsigned,
                responder,
                |_| -> Result<_, V2BlockSyncError> {
                    panic!("changed unsigned request must not read history")
                },
            ),
            Err(V2BlockSyncError::ConflictingHistoricalBodyRequest {
                existing,
                incoming,
            }) if existing == restarted_hash && incoming == changed_hash
        ));
        assert_eq!(server.body_len(), 1);
        assert_eq!(server.body_response_bytes(), rebound_bytes);
    }
    #[test]
    fn oversized_historical_body_response_is_served_without_caching() {
        let fixture = Fixture::new();
        let request = fixture.body_request(fixture.artifact.commit_qc.clone());
        let response = body_cache_response(&fixture, &request);
        let response_bytes = response.encoded_len();
        let mut server = V2BlockSyncServer::new_with_body_response_byte_capacity(
            fixture.context.network_id,
            2,
            response_bytes.checked_sub(1).expect("non-empty response"),
        )
        .expect("byte-bounded body server");
        let builds = Cell::new(0_u32);
        for expected_builds in 1..=2 {
            let served = server
                .serve_historical_body_with(
                    request.clone(),
                    &peer(&fixture.requester),
                    &fixture.old_validators[0],
                    |_| {
                        builds.set(builds.get() + 1);
                        Ok(Some(response.clone()))
                    },
                )
                .expect("oversized response remains serviceable")
                .expect("builder supplied a response");
            assert_eq!(served, response);
            assert_eq!(builds.get(), expected_builds);
            assert_eq!(server.body_len(), 0);
            assert_eq!(server.body_response_bytes(), 0);
        }
    }
    #[test]
    fn historical_body_response_cache_evicts_to_aggregate_byte_cap() {
        let fixture = Fixture::new();
        let first_request = fixture.body_request(fixture.artifact.commit_qc.clone());
        let second_requester = key(92);
        let mut second_request = first_request.clone();
        second_request.requester = peer(&second_requester);
        second_request.signature = Signature::new(
            second_requester.private_key(),
            &second_request.signature_preimage(),
        )
        .payload()
        .to_vec();
        let first_response = body_cache_response(&fixture, &first_request);
        let second_response = body_cache_response(&fixture, &second_request);
        let first_bytes = first_response.encoded_len();
        let second_bytes = second_response.encoded_len();
        let byte_capacity = first_bytes.max(second_bytes);
        let mut server = V2BlockSyncServer::new_with_body_response_byte_capacity(
            fixture.context.network_id,
            2,
            byte_capacity,
        )
        .expect("byte-bounded body server");
        let first = server
            .serve_historical_body_with(
                first_request.clone(),
                &peer(&fixture.requester),
                &fixture.old_validators[0],
                |_| Ok(Some(first_response.clone())),
            )
            .expect("serve first response")
            .expect("first response exists");
        assert_eq!(first, first_response);
        assert_eq!(server.body_len(), 1);
        assert_eq!(server.body_response_bytes(), first_bytes);
        let replay = server
            .serve_historical_body_with(
                first_request.clone(),
                &peer(&fixture.requester),
                &fixture.old_validators[0],
                |_| -> Result<_, V2BlockSyncError> {
                    panic!("an exact cache hit must not rebuild the response")
                },
            )
            .expect("serve cached response")
            .expect("cached response exists");
        assert_eq!(replay, first_response);
        assert_eq!(server.body_response_bytes(), first_bytes);
        let second = server
            .serve_historical_body_with(
                second_request,
                &peer(&second_requester),
                &fixture.old_validators[0],
                |_| Ok(Some(second_response.clone())),
            )
            .expect("serve second response")
            .expect("second response exists");
        assert_eq!(second, second_response);
        assert_eq!(server.body_len(), 1);
        assert_eq!(server.body_response_bytes(), second_bytes);
        assert!(server.body_response_bytes() <= byte_capacity);
        let rebuilt = Cell::new(false);
        let first_again = server
            .serve_historical_body_with(
                first_request,
                &peer(&fixture.requester),
                &fixture.old_validators[0],
                |_| {
                    rebuilt.set(true);
                    Ok(Some(first_response.clone()))
                },
            )
            .expect("serve evicted first response")
            .expect("rebuilt response exists");
        assert!(rebuilt.get());
        assert_eq!(first_again, first_response);
        assert_eq!(server.body_len(), 1);
        assert_eq!(server.body_response_bytes(), first_bytes);
        assert!(server.body_response_bytes() <= byte_capacity);
    }
    #[test]
    fn authenticated_prepare_qc_serves_only_exact_finalized_kura_body() {
        let history = durable_history_fixture();
        let context = history.artifact.height_context.clone();
        let requester_key = key(90);
        assert_eq!(peer(&requester_key), history.requester);
        let sign_body_request = |certificate: wire::QuorumCertificate| {
            let mut request = wire::CertifiedBodyRequest {
                round: certificate.proposal_round,
                subject: certificate.subject,
                certificate,
                requester: history.requester.clone(),
                signature: Vec::new(),
            };
            request.signature =
                Signature::new(requester_key.private_key(), &request.signature_preimage())
                    .payload()
                    .to_vec();
            request
        };
        let mut prepare_qc = history.artifact.commit_qc.clone();
        prepare_qc.phase = wire::GlobalPhase::Prepare;
        prepare_qc.aggregate_signature = aggregate_certificate(&prepare_qc, &history.validators);
        let request = sign_body_request(prepare_qc.clone());
        let request_hash = HashOf::new(&request);
        let mut server =
            V2BlockSyncServer::new(context.network_id.clone(), 1).expect("body server");
        let response = server
            .serve_historical_body(
                history.kura.as_ref(),
                request,
                &history.requester,
                &history.validators[3],
            )
            .expect("authenticated PrepareQC request is valid")
            .expect("exact finalized PrepareQC subject is served from Kura");
        let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = response.payload
        else {
            panic!("historical PrepareQC service emits a certified body response")
        };
        let canonical_body = history
            .kura
            .get_block(NonZeroUsize::new(1).expect("non-zero height"))
            .expect("canonical historical block")
            .canonical_resultless_proposal()
            .encode_wire()
            .expect("canonical resultless proposal wire");
        assert_eq!(response.request_hash, request_hash);
        assert_eq!(response.body, canonical_body);
        assert_eq!(response.manifest.round, prepare_qc.proposal_round);
        assert_eq!(response.manifest.subject, history.artifact.subject);
        assert_eq!(response.responder, peer(&history.validators[3]));
        assert_eq!(server.body_len(), 1);
        let mut mismatched_qc = prepare_qc.clone();
        mismatched_qc.subject.payload_hash = Hash::new(b"non-canonical historical payload");
        mismatched_qc.aggregate_signature =
            aggregate_certificate(&mismatched_qc, &history.validators);
        let mut mismatch_server =
            V2BlockSyncServer::new(context.network_id.clone(), 1).expect("mismatch server");
        assert!(matches!(
            mismatch_server.serve_historical_body(
                history.kura.as_ref(),
                sign_body_request(mismatched_qc),
                &history.requester,
                &history.validators[0],
            ),
            Err(V2BlockSyncError::HistoricalSubjectMismatch { height: 1 })
        ));
        assert_eq!(mismatch_server.body_len(), 0);
        let mut invalid_qc = prepare_qc;
        invalid_qc.aggregate_signature = vec![0xEE; 96];
        let mut invalid_server =
            V2BlockSyncServer::new(context.network_id.clone(), 1).expect("invalid-proof server");
        assert!(matches!(
            invalid_server.serve_historical_body(
                history.kura.as_ref(),
                sign_body_request(invalid_qc),
                &history.requester,
                &history.validators[0],
            ),
            Err(V2BlockSyncError::Transport(
                V2TransportError::CertificateRejected(_)
            ))
        ));
        assert_eq!(invalid_server.body_len(), 0);
    }
    #[test]
    fn historical_body_uses_self_contained_kura_finality_without_context_store() {
        let fixture = Fixture::new();
        let committed = ValidBlock::new_dummy_and_modify_header(
            fixture.old_validators[0].private_key(),
            |header| {
                header.set_height(NonZeroU64::new(1).expect("non-zero height"));
                header.set_prev_block_hash(None);
                header.set_view_change_index(4);
                header.merkle_root = None;
            },
        )
        .commit_unchecked()
        .unpack(|_| {});
        let mut executed_block: iroha_data_model::block::SignedBlock = committed.into();
        executed_block
            .set_transaction_results(Vec::new(), &[], Vec::new())
            .expect("attach an empty deterministic execution result");
        assert!(!executed_block.is_resultless_proposal());
        let executed_block_wire = executed_block
            .encode_wire()
            .expect("canonical executed block wire");
        let executed_block_wire_len =
            u64::try_from(executed_block_wire.len()).expect("executed wire length fits u64");
        let executed_block_wire_hash = Hash::new(&executed_block_wire);
        let proposal = executed_block.canonical_resultless_proposal();
        let canonical_wire = proposal
            .encode_wire()
            .expect("canonical proposal block wire");
        assert!(proposal.is_resultless_proposal());
        let block = Arc::new(executed_block);
        let mut context = fixture.context.clone();
        context.da_layout = wire::SumeragiV2GenesisContextParameters::recommended().da_layout;
        context.validate().expect("historical context");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let mut exact_execution_commitment = execution_commitment(0x43);
        exact_execution_commitment.executed_block_wire_len = executed_block_wire_len;
        exact_execution_commitment.executed_block_wire_hash = executed_block_wire_hash;
        let mut certificate = wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: 1,
                view: 4,
            },
            proposal_round: wire::ConsensusRound {
                context_id: context.id(),
                height: 1,
                view: 4,
            },
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: exact_execution_commitment,
            signers: vec![0, 1, 2],
            aggregate_signature: Vec::new(),
        };
        certificate.aggregate_signature =
            aggregate_certificate(&certificate, &fixture.old_validators);
        let artifact = wire::finality::V2FinalityArtifact::new(
            context.clone(),
            subject,
            certificate.clone(),
            fixture.proofs_of_possession.clone(),
        );
        let kura = Kura::blank_kura_for_testing();
        kura.store_block(block).expect("store canonical block");
        let _ = kura
            .store_v2_finality_artifact(&artifact)
            .expect("store finality artifact");
        assert!(
            !kura.sumeragi_v2_storage_root().join("contexts").exists(),
            "the regression must not create a duplicate historical context record"
        );
        let request = fixture.body_request(certificate.clone());
        let request_hash = HashOf::new(&request);
        let mut server =
            V2BlockSyncServer::new(context.network_id.clone(), 2).expect("sync server");
        let response = server
            .serve_historical_body(
                kura.as_ref(),
                request.clone(),
                &peer(&fixture.requester),
                &fixture.old_validators[0],
            )
            .expect("serve historical body")
            .expect("frozen-roster signer retained canonical Kura body");
        assert_eq!(server.body_len(), 1);
        let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = response.payload
        else {
            panic!("historical service emits certified body response")
        };
        assert_eq!(response.request_hash, request_hash);
        assert_eq!(response.body, canonical_wire);
        let decoded_response = iroha_data_model::block::decode_framed_signed_block(&response.body)
            .expect("decode historical response proposal");
        assert!(decoded_response.is_resultless_proposal());
        assert_eq!(response.manifest.round, certificate.proposal_round);
        assert_eq!(response.manifest.subject, subject);
        assert_eq!(response.responder, peer(&fixture.old_validators[0]));
        let replay = server
            .serve_historical_body_with(
                request.clone(),
                &peer(&fixture.requester),
                &fixture.old_validators[0],
                |_| -> Result<_, V2BlockSyncError> {
                    panic!("exact historical body replay must not read Kura")
                },
            )
            .expect("cached historical body replay")
            .expect("cached historical response");
        assert!(matches!(
            replay.payload,
            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(ref cached)
                if cached == &response
        ));
        let authenticated_request = authenticate_certified_body_request(
            &context,
            request.clone(),
            &peer(&fixture.requester),
            |context, certificate| {
                verify_historical_quorum_certificate(
                    context,
                    &fixture.proofs_of_possession,
                    certificate,
                )
            },
        )
        .expect("authenticate exact request");
        let mut outstanding =
            OutstandingCertifiedBodyRequests::new(1).expect("bounded response tracker");
        outstanding
            .register(authenticated_request)
            .expect("register request");
        let request_hash = HashOf::new(&request);
        let _ = outstanding
            .authenticate_response(&context, response, &peer(&fixture.old_validators[0]))
            .expect("lagging peer accepts exact certified Kura body");
        assert!(outstanding.contains(request_hash));
        assert!(outstanding.complete(request_hash));
        assert!(outstanding.is_empty());
        let archive_response = server
            .serve_historical_body(
                kura.as_ref(),
                request.clone(),
                &peer(&fixture.requester),
                &fixture.old_validators[3],
            )
            .expect("serve from applied frozen-roster archive")
            .expect("non-QC-signer archive serves exact Kura body");
        let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(archive_response) =
            archive_response.payload
        else {
            panic!("historical archive emits certified body response")
        };
        assert_eq!(archive_response.responder, peer(&fixture.old_validators[3]));
        assert_eq!(archive_response.request_hash, request_hash);
        assert_eq!(archive_response.body, canonical_wire);
        let authenticated_archive_request = authenticate_certified_body_request(
            &context,
            request.clone(),
            &peer(&fixture.requester),
            |context, certificate| {
                verify_historical_quorum_certificate(
                    context,
                    &fixture.proofs_of_possession,
                    certificate,
                )
            },
        )
        .expect("authenticate archive request");
        let mut archive_outstanding =
            OutstandingCertifiedBodyRequests::new(1).expect("bounded archive tracker");
        archive_outstanding
            .register(authenticated_archive_request)
            .expect("register archive request");
        let authenticated_archive_response = archive_outstanding
            .authenticate_response(
                &context,
                archive_response,
                &peer(&fixture.old_validators[3]),
            )
            .expect("lagging peer accepts non-QC-signer archive response");
        assert_eq!(
            authenticated_archive_response.response().request_hash,
            request_hash
        );
        assert_eq!(
            authenticated_archive_response.response().responder,
            peer(&fixture.old_validators[fixture.old_validators.len() - 1])
        );
        assert_eq!(
            authenticated_archive_response.response().body,
            canonical_wire
        );
        assert!(archive_outstanding.contains(request_hash));
        assert!(archive_outstanding.complete(request_hash));
        assert!(archive_outstanding.is_empty());
        assert_eq!(server.body_len(), 1);
        let rotated_response = server
            .serve_historical_body(
                kura.as_ref(),
                request.clone(),
                &peer(&fixture.requester),
                &fixture.rotated_responder,
            )
            .expect("serve with current rotated archive key")
            .expect("rotated archive serves exact historical Kura body");
        let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(rotated_response) =
            rotated_response.payload
        else {
            panic!("rotated archive emits certified body response")
        };
        let rotated_peer = peer(&fixture.rotated_responder);
        assert_eq!(rotated_response.responder, rotated_peer);
        assert_eq!(rotated_response.request_hash, request_hash);
        assert_eq!(rotated_response.body, canonical_wire);
        let rotated_request = authenticate_certified_body_request(
            &context,
            request.clone(),
            &peer(&fixture.requester),
            |context, certificate| {
                verify_historical_quorum_certificate(
                    context,
                    &fixture.proofs_of_possession,
                    certificate,
                )
            },
        )
        .expect("authenticate rotated archive request");
        let authenticated_rotated_response = rotated_request
            .authenticate_response(&context, rotated_response.clone(), &rotated_peer)
            .expect("lagging peer accepts current key for exact historical body");
        assert_eq!(authenticated_rotated_response.response(), &rotated_response);
        assert_eq!(server.body_len(), 1);
        let mut forged = request;
        forged.certificate.aggregate_signature = vec![0xEE; 96];
        forged.signature = Signature::new(
            fixture.requester.private_key(),
            &forged.signature_preimage(),
        )
        .payload()
        .to_vec();
        let mut invalid_server =
            V2BlockSyncServer::new(context.network_id.clone(), 1).expect("invalid-proof server");
        assert!(matches!(
            invalid_server.serve_historical_body(
                kura.as_ref(),
                forged,
                &peer(&fixture.requester),
                &fixture.old_validators[0],
            ),
            Err(V2BlockSyncError::Transport(
                V2TransportError::CertificateRejected(_)
            ))
        ));
        assert_eq!(invalid_server.body_len(), 0);
        assert_eq!(server.body_len(), 1);
    }
}
