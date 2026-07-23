//! Reducer-preserving sequential block synchronization for Sumeragi v2.
//!
//! This module discovers the CommitQC for the caller's already-frozen active
//! [`wire::HeightContext`] and lets a certified historical signer serve the
//! exact canonical body after active peers have rolled forward. It never
//! imports a block, writes Kura, forms a certificate, or changes height
//! directly. A successfully authenticated certificate response is converted
//! to the ordinary v2 `QuorumCertificate` envelope and must be admitted through
//! [`super::v2_effects::V2EffectExecutor`]. The sole reducer then persists its
//! decision, requests the body, validates and stores the response, and applies
//! it through the normal WAL path.
//!
//! Responders load historical immutable finality artifacts from Kura. Their
//! response signature uses their current P2P identity, so validator key
//! rotation does not make a retired height unservable. The historical CommitQC
//! remains independently verified under the height's frozen roster.

use core::fmt;
use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroUsize,
};

use iroha_crypto::{Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{ChainId, block::consensus_v2 as wire, peer::PeerId};
use thiserror::Error;

use super::v2_transport::{
    AuthenticatedCertifiedBodyRequest, AuthenticatedCommitCertificateResponse,
    OutstandingCommitCertificateRequests, V2TransportError, authenticate_certified_body_request,
    authenticate_certified_body_request_identity, authenticate_commit_certificate_request,
    authenticate_commit_certificate_request_identity,
};
use super::{
    v2::verify_persisted_quorum_certificate,
    v2_chunks::encode_payload,
    v2_context_store::V2ContextStore,
    v2_core::{
        CanonicalIdentityProjection, IDENTITY_DOMAIN_CONTEXT, IDENTITY_DOMAIN_PAYLOAD,
        IDENTITY_KIND_COMMIT_CERTIFICATE_REQUEST, IDENTITY_KIND_CONSENSUS_MESSAGE,
        IDENTITY_KIND_QUORUM_CERTIFICATE, IDENTITY_KIND_WIRE_HEIGHT_CONTEXT,
        ProductionHistoricalCertificateTraceProjection,
        production_historical_certificate_trace_refines_indexed_async_kernel,
    },
    v2_effects::CommitCertificateReducerAdmission,
};
use crate::kura::Kura;

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
/// read. A differently signed request for the same logical
/// `(context,height,requester)` is rejected while cached, preventing replay
/// churn from multiplying Kura work. FIFO eviction is safe because these are
/// transport responses, never consensus state; evicted clients retransmit the
/// same exact request. A serving-key rotation invalidates an old cached
/// response before reuse so the signed identity always matches the current
/// authenticated outer peer.
pub(crate) struct V2BlockSyncServer {
    chain_id: ChainId,
    capacity: usize,
    responses: BTreeMap<HashOf<wire::CommitCertificateRequest>, wire::ConsensusMessageV2>,
    identities: BTreeMap<CommitCertificateServerIdentity, HashOf<wire::CommitCertificateRequest>>,
    order: VecDeque<HashOf<wire::CommitCertificateRequest>>,
    body_responses: BTreeMap<HashOf<wire::CertifiedBodyRequest>, CachedHistoricalBodyResponse>,
    body_identities: BTreeMap<HistoricalBodyRequestIdentity, HashOf<wire::CertifiedBodyRequest>>,
    body_order: VecDeque<HashOf<wire::CertifiedBodyRequest>>,
}

impl V2BlockSyncServer {
    /// Construct an empty bounded server for one configured chain.
    pub(crate) fn new(chain_id: ChainId, capacity: usize) -> Result<Self, V2BlockSyncError> {
        if capacity == 0 {
            return Err(V2TransportError::ZeroCapacity.into());
        }
        Ok(Self {
            chain_id,
            capacity,
            responses: BTreeMap::new(),
            identities: BTreeMap::new(),
            order: VecDeque::new(),
            body_responses: BTreeMap::new(),
            body_identities: BTreeMap::new(),
            body_order: VecDeque::new(),
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

    /// Serve an exact historical canonical body only when this node is one of
    /// the request QC's certified signers under that height's frozen roster.
    ///
    /// The historical context and PoPs are loaded from the immutable context
    /// store, while Kura supplies both the canonical finality artifact and
    /// exact block bytes. The receiver still stores and validates the returned
    /// body through its active reducer effects; this service never imports or
    /// applies a block locally.
    pub(crate) fn serve_historical_body(
        &mut self,
        kura: &Kura,
        context_store: &V2ContextStore,
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
                    context_store,
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
        let request_hash = HashOf::new(&request);
        let responder = PeerId::new(responder_key.public_key().clone());
        if let Some(cached) = self.body_responses.get(&request_hash) {
            if cached.responder == responder {
                return Ok(Some(cached.message.clone()));
            }
            self.remove_body(request_hash);
        }
        let identity = HistoricalBodyRequestIdentity::from(&request);
        if let Some(existing) = self.body_identities.get(&identity) {
            return Err(V2BlockSyncError::ConflictingHistoricalBodyRequest {
                existing: *existing,
                incoming: request_hash,
            });
        }

        let Some(response) = build(&request)? else {
            return Ok(None);
        };
        while self.body_responses.len() >= self.capacity {
            let Some(oldest) = self.body_order.pop_front() else {
                return Err(V2BlockSyncError::CorruptServerCache);
            };
            self.remove_body(oldest);
        }
        self.body_responses.insert(
            request_hash,
            CachedHistoricalBodyResponse {
                responder,
                message: response.clone(),
            },
        );
        self.body_identities.insert(identity, request_hash);
        self.body_order.push_back(request_hash);
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
        if request.chain_id != self.chain_id || request.height == 0 {
            return Err(wire::ValidationError::WrongHeightContext.into());
        }
        let request_hash = HashOf::new(&request);
        let responder = PeerId::new(responder_key.public_key().clone());
        if let Some(cached) = self.responses.get(&request_hash) {
            let current_identity = matches!(
                &cached.payload,
                wire::ConsensusMessageV2Payload::CommitCertificateResponse(response)
                    if response.responder == responder
            );
            if current_identity {
                return Ok(Some(cached.clone()));
            }
            self.remove(request_hash);
        }

        let identity = CommitCertificateServerIdentity::from(&request);
        if let Some(existing) = self.identities.get(&identity) {
            return Err(V2BlockSyncError::ConflictingServerRequest {
                existing: *existing,
                incoming: request_hash,
            });
        }

        let Some(response) = build(&request)? else {
            return Ok(None);
        };
        while self.responses.len() >= self.capacity {
            let Some(oldest) = self.order.pop_front() else {
                return Err(V2BlockSyncError::CorruptServerCache);
            };
            self.remove(oldest);
        }
        self.responses.insert(request_hash, response.clone());
        self.identities.insert(identity, request_hash);
        self.order.push_back(request_hash);
        Ok(Some(response))
    }

    fn remove(&mut self, request_hash: HashOf<wire::CommitCertificateRequest>) {
        let Some(message) = self.responses.remove(&request_hash) else {
            return;
        };
        let wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) = message.payload
        else {
            debug_assert!(false, "v2 sync cache contains a non-response payload");
            return;
        };
        // The response hash identifies the request, but the logical identity
        // lives only in the request. Locate one bounded reverse entry.
        self.identities
            .retain(|_, hash| *hash != response.request_hash);
        self.order.retain(|hash| *hash != request_hash);
    }

    fn remove_body(&mut self, request_hash: HashOf<wire::CertifiedBodyRequest>) {
        if self.body_responses.remove(&request_hash).is_none() {
            return;
        }
        self.body_identities.retain(|_, hash| *hash != request_hash);
        self.body_order.retain(|hash| *hash != request_hash);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.responses.len()
    }

    #[cfg(test)]
    fn body_len(&self) -> usize {
        self.body_responses.len()
    }
}

#[derive(Clone, Debug)]
struct CachedHistoricalBodyResponse {
    responder: PeerId,
    message: wire::ConsensusMessageV2,
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
            chain_id: self.context.chain_id.clone(),
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
        let admission =
            enqueue(message.clone()).map_err(CommitCertificateAdmissionError::Enqueue)?;
        if !admission.matches(&message) {
            return Err(CommitCertificateAdmissionError::MismatchedReducerAdmission);
        }
        let admitted_message_hash = admission.refinement_projection();
        if !self.complete(discovered) {
            return Err(CommitCertificateAdmissionError::RequestDisappeared);
        }
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
            message_hash: historical_typed_identity(
                IDENTITY_DOMAIN_PAYLOAD,
                IDENTITY_KIND_CONSENSUS_MESSAGE,
                HashOf::new(&message),
            ),
            admitted_message_hash,
            request_present_before,
            request_present_after: self.outstanding.contains(request_hash),
        };
        if !production_historical_certificate_trace_refines_indexed_async_kernel(historical_trace) {
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
    context_store: &V2ContextStore,
    request: wire::CertifiedBodyRequest,
    authenticated_requester: &PeerId,
    responder_key: &KeyPair,
) -> Result<Option<wire::ConsensusMessageV2>, V2BlockSyncError> {
    let height = request.round.height;
    let Some(artifact) = kura.v2_finality_artifact(height)? else {
        return Ok(None);
    };
    let persisted = context_store
        .load(height)?
        .ok_or(V2BlockSyncError::MissingHistoricalContext { height })?;
    if persisted.context() != &artifact.height_context {
        return Err(V2BlockSyncError::HistoricalContextMismatch { height });
    }
    let authenticated: AuthenticatedCertifiedBodyRequest = authenticate_certified_body_request(
        persisted.context(),
        request,
        authenticated_requester,
        |context, certificate| {
            verify_persisted_quorum_certificate(
                context,
                persisted.proofs_of_possession(),
                certificate,
            )
        },
    )?;
    let request = authenticated.request();
    if request.certificate.phase != wire::GlobalPhase::Commit {
        return Ok(None);
    }
    if request.subject != artifact.subject {
        return Err(V2BlockSyncError::HistoricalSubjectMismatch { height });
    }

    let responder_peer = PeerId::new(responder_key.public_key().clone());
    let Some(responder_position) = persisted
        .context()
        .roster
        .iter()
        .position(|entry| entry.validator == responder_peer)
    else {
        return Ok(None);
    };
    let responder = u32::try_from(responder_position)?;
    if request
        .certificate
        .signers
        .binary_search(&responder)
        .is_err()
    {
        return Ok(None);
    }

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
    // authenticated by the CommitQC subject.
    let proposal = block.canonical_resultless_proposal();
    let body = proposal
        .encode_wire()
        .map_err(|error| V2BlockSyncError::CanonicalBody(error.to_string()))?;
    if !proposal.is_resultless_proposal() || Hash::new(&body) != request.subject.payload_hash {
        return Err(V2BlockSyncError::HistoricalSubjectMismatch { height });
    }
    let encoded = encode_payload(persisted.context(), request.round, request.subject, &body)?;
    let (manifest, _) = encoded.into_parts();
    let mut response = wire::CertifiedBodyResponse {
        request_hash: authenticated.request_hash(),
        manifest,
        body,
        responder,
        signature: Vec::new(),
    };
    response.signature =
        Signature::new(responder_key.private_key(), &response.signature_preimage())
            .payload()
            .to_vec();
    response.validate_against(persisted.context(), request, &responder_peer)?;
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
    /// Immutable historical context or PoP record could not be loaded.
    #[error(transparent)]
    ContextStore(#[from] super::v2_context_store::V2ContextStoreError),
    /// Canonical body chunking under the frozen DA layout failed.
    #[error(transparent)]
    Chunk(#[from] super::v2_chunks::V2ChunkError),
    /// A bounded height/index conversion overflowed local representation.
    #[error(transparent)]
    Integer(#[from] std::num::TryFromIntError),
    /// The supplied signing key is not the configured local P2P identity.
    #[error("Sumeragi v2 block-sync signing key differs from the configured local peer")]
    LocalKeyMismatch,
    /// A differently signed request attempted to reuse one cached logical slot.
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
    /// Kura finality exists but its immutable context/PoP record is absent.
    #[error("Sumeragi v2 historical context is missing at height {height}")]
    MissingHistoricalContext {
        /// Requested historical height.
        height: wire::Height,
    },
    /// Kura finality and the immutable context record disagree.
    #[error("Sumeragi v2 historical context differs from finality at height {height}")]
    HistoricalContextMismatch {
        /// Conflicting historical height.
        height: wire::Height,
    },
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
    /// A differently signed body request attempted to reuse one cached logical slot.
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
    use std::{cell::Cell, num::NonZeroU64, sync::Arc};

    use iroha_crypto::{Algorithm, Hash};
    use iroha_data_model::{ChainId, block::BlockHeader};

    use super::*;
    use crate::{
        block::ValidBlock,
        sumeragi::{
            v2::VerifiedHeightContext, v2_context_store::PersistedHeightContext,
            v2_transport::OutstandingCertifiedBodyRequests,
        },
    };

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
                chain_id: ChainId::from("v2-block-sync-test"),
                protocol_version: wire::PROTOCOL_VERSION,
                height: 1,
                epoch: 0,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum: wire::DualQuorum::from_roster(&roster).expect("dual quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"v2 sync nexus/amx context"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::Plain,
                    chunk_size_bytes: 1024,
                    data_shards: 0,
                    parity_shards: 0,
                    max_payload_size_bytes: 4096,
                    max_chunk_count: 4,
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
            commit_qc.aggregate_signature = aggregate_commit(&commit_qc, &old_validators);
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
        wire::ExecutionCommitment::without_topups(
            Hash::new([seed, 1]),
            Hash::new([seed, 2]),
            Hash::new([seed, 3]),
            Hash::new([seed, 4]),
        )
    }

    fn aggregate_commit(certificate: &wire::QuorumCertificate, keys: &[KeyPair]) -> Vec<u8> {
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
        iroha_crypto::bls_normal_aggregate_signatures(&refs).expect("aggregate CommitQC")
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
        let executed_block_wire_hash = executed_block
            .executed_block_wire_hash()
            .expect("encode executed history-fixture block");
        let proposal = executed_block.canonical_resultless_proposal();
        let canonical_wire = proposal
            .encode_wire()
            .expect("encode history-fixture proposal");
        let block = Arc::new(executed_block);
        let mut context = fixture.context.clone();
        context.da_layout.max_payload_size_bytes = 1_048_576;
        context.da_layout.max_chunk_count = 1024;
        context.validate().expect("valid history-fixture context");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let mut execution = execution_commitment(0x43);
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
        certificate.aggregate_signature = aggregate_commit(&certificate, &fixture.old_validators);
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
        let context_store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("history context store");
        let verified =
            VerifiedHeightContext::genesis(context.clone(), fixture.proofs_of_possession.clone())
                .expect("verify history-fixture context");
        context_store
            .persist(&PersistedHeightContext::from_verified(&verified))
            .expect("persist history-fixture context");

        let requester = peer(&fixture.requester);
        let mut commit_request = wire::CommitCertificateRequest {
            protocol_version: wire::PROTOCOL_VERSION,
            chain_id: context.chain_id.clone(),
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
            &fixture.old_validators[0],
        )
        .expect("build durable CommitQC response");
        let body_request = fixture.body_request(certificate);
        let body_response = build_historical_body_response(
            kura.as_ref(),
            &context_store,
            body_request,
            &requester,
            &fixture.old_validators[0],
        )
        .expect("build durable body response")
        .expect("history fixture has a certified responder");
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
        cross_chain.chain_id = ChainId::from("another-chain");
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
        let mut server =
            V2BlockSyncServer::new(fixture.context.chain_id.clone(), 1).expect("server");
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
                |request| {
                    builds.set(builds.get() + 1);
                    serve_commit_certificate_from_artifact(
                        &fixture.artifact,
                        request.clone(),
                        &peer(&fixture.requester),
                        &new_responder,
                    )
                    .map(Some)
                },
            )
            .expect("re-sign after key rotation")
            .expect("rotated response");
        assert_eq!(builds.get(), 2);
        assert_ne!(rotated, first);
        let wire::ConsensusMessageV2Payload::CommitCertificateResponse(rotated) = rotated.payload
        else {
            panic!("response payload")
        };
        assert_eq!(rotated.responder, peer(&new_responder));

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
    fn server_returns_none_for_missing_canonical_artifact_without_caching() {
        let fixture = Fixture::new();
        let mut discovery = fixture.discovery();
        let request = fixture.signed_request(&mut discovery);
        let kura = Kura::blank_kura_for_testing();
        let mut server =
            V2BlockSyncServer::new(fixture.context.chain_id.clone(), 2).expect("server");

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
            V2BlockSyncServer::new(fixture.context.chain_id.clone(), 2).expect("server");

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
        cross_chain.chain_id = ChainId::from("cross-chain-replay");
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
            V2BlockSyncServer::new(fixture.context.chain_id.clone(), 2).expect("server");
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
    fn historical_body_comes_from_kura_and_only_a_certified_signer_can_serve() {
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
        let executed_block_wire_hash = executed_block
            .executed_block_wire_hash()
            .expect("canonical executed block wire");
        let proposal = executed_block.canonical_resultless_proposal();
        let canonical_wire = proposal
            .encode_wire()
            .expect("canonical proposal block wire");
        assert!(proposal.is_resultless_proposal());
        let block = Arc::new(executed_block);
        let mut context = fixture.context.clone();
        context.da_layout.max_payload_size_bytes = 1_048_576;
        context.da_layout.max_chunk_count = 1024;
        context.validate().expect("historical context");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let mut exact_execution_commitment = execution_commitment(0x43);
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
        certificate.aggregate_signature = aggregate_commit(&certificate, &fixture.old_validators);
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
        let context_store =
            V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("context store");
        let verified =
            VerifiedHeightContext::genesis(context.clone(), fixture.proofs_of_possession.clone())
                .expect("verified historical context");
        context_store
            .persist(&PersistedHeightContext::from_verified(&verified))
            .expect("persist historical context");

        let request = fixture.body_request(certificate.clone());
        let request_hash = HashOf::new(&request);
        let mut server = V2BlockSyncServer::new(context.chain_id.clone(), 2).expect("sync server");
        let response = server
            .serve_historical_body(
                kura.as_ref(),
                &context_store,
                request.clone(),
                &peer(&fixture.requester),
                &fixture.old_validators[0],
            )
            .expect("serve historical body")
            .expect("certified signer retained canonical Kura body");
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
        assert_eq!(response.responder, 0);
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
                verify_persisted_quorum_certificate(
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

        assert!(
            server
                .serve_historical_body(
                    kura.as_ref(),
                    &context_store,
                    request.clone(),
                    &peer(&fixture.requester),
                    &fixture.old_validators[3],
                )
                .expect("non-signer is safely ignored")
                .is_none()
        );
        assert_eq!(server.body_len(), 0);
        assert!(
            server
                .serve_historical_body(
                    kura.as_ref(),
                    &context_store,
                    request.clone(),
                    &peer(&fixture.requester),
                    &fixture.rotated_responder,
                )
                .expect("rotated non-historical key is safely ignored")
                .is_none()
        );

        let mut forged = request;
        forged.certificate.aggregate_signature = vec![0xEE; 96];
        forged.signature = Signature::new(
            fixture.requester.private_key(),
            &forged.signature_preimage(),
        )
        .payload()
        .to_vec();
        assert!(matches!(
            server.serve_historical_body(
                kura.as_ref(),
                &context_store,
                forged,
                &peer(&fixture.requester),
                &fixture.old_validators[0],
            ),
            Err(V2BlockSyncError::Transport(
                V2TransportError::CertificateRejected(_)
            ))
        ));
        assert_eq!(server.body_len(), 0);
    }
}
