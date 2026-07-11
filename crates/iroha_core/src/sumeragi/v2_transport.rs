//! Authenticated transport boundary for Sumeragi v2 payload dissemination.
//!
//! The consensus reducer accepts only already-authenticated control inputs. This
//! module applies the same rule to payload chunks and certified body fetches:
//! structural wire validation, transport identity binding, and cryptographic
//! authentication all complete before an adapter may act on the payload.

use core::fmt;
use std::collections::BTreeMap;

use iroha_crypto::{HashOf, Signature};
use iroha_data_model::{block::consensus_v2 as wire, peer::PeerId};

/// Kind of signed transport payload rejected during authentication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransportSignatureKind {
    /// An encoded payload chunk.
    PayloadChunk,
    /// A certified-body request.
    CertifiedBodyRequest,
    /// A certified-body response.
    CertifiedBodyResponse,
    /// A request for one historical height's durable CommitQC.
    CommitCertificateRequest,
    /// A response carrying one historical height's durable CommitQC.
    CommitCertificateResponse,
}

/// Claimed identity whose binding to the authenticated outer peer failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransportIdentityKind {
    /// The validator index carried by a payload chunk.
    ChunkSender,
    /// The peer identity carried by a certified-body request.
    Requester,
    /// The validator index carried by a certified-body response.
    Responder,
    /// The peer identity carried by a commit-certificate request.
    CommitCertificateRequester,
    /// The current peer identity carried by a commit-certificate response.
    CommitCertificateResponder,
}

/// Authentication or outstanding-request tracking failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum V2TransportError {
    /// A wire value violated its structural contract.
    Wire(wire::ValidationError),
    /// The authenticated outer transport peer did not match the signed claim.
    OuterIdentityMismatch {
        /// Kind of identity being bound.
        kind: TransportIdentityKind,
        /// Identity carried by the signed payload.
        claimed: PeerId,
        /// Identity supplied by the authenticated transport.
        authenticated: PeerId,
    },
    /// A payload signature was malformed or did not verify.
    InvalidSignature {
        /// Kind of payload carrying the rejected signature.
        kind: TransportSignatureKind,
        /// Cryptographic parser or verifier diagnostic.
        reason: String,
    },
    /// The caller-supplied quorum-certificate verifier rejected a request.
    CertificateRejected(String),
    /// An outstanding-request tracker was constructed with zero capacity.
    ZeroCapacity,
    /// The exact request is already outstanding.
    DuplicateRequest(HashOf<wire::CertifiedBodyRequest>),
    /// Another exact request already occupies the same logical request slot.
    ConflictingRequest {
        /// Hash of the existing request.
        existing: HashOf<wire::CertifiedBodyRequest>,
        /// Hash of the newly presented request.
        incoming: HashOf<wire::CertifiedBodyRequest>,
    },
    /// Registering another request would exceed the explicit capacity bound.
    CapacityExceeded {
        /// Configured maximum number of requests.
        capacity: usize,
    },
    /// A response did not match any currently outstanding exact request hash.
    UnsolicitedResponse(HashOf<wire::CertifiedBodyRequest>),
    /// The exact commit-certificate request is already outstanding.
    DuplicateCommitCertificateRequest(HashOf<wire::CommitCertificateRequest>),
    /// Another signed request already occupies this exact context/requester slot.
    ConflictingCommitCertificateRequest {
        /// Hash of the request already occupying the slot.
        existing: HashOf<wire::CommitCertificateRequest>,
        /// Hash of the conflicting signed request.
        incoming: HashOf<wire::CommitCertificateRequest>,
    },
    /// The bounded commit-certificate discovery table is full.
    CommitCertificateCapacityExceeded {
        /// Configured maximum number of outstanding requests.
        capacity: usize,
    },
    /// A commit-certificate response is unsolicited, late, or replayed.
    UnsolicitedCommitCertificateResponse(HashOf<wire::CommitCertificateRequest>),
}

impl fmt::Display for V2TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => write!(f, "invalid Sumeragi v2 transport payload: {error}"),
            Self::OuterIdentityMismatch {
                kind,
                claimed,
                authenticated,
            } => write!(
                f,
                "{kind:?} identity {claimed} differs from authenticated transport peer {authenticated}"
            ),
            Self::InvalidSignature { kind, reason } => {
                write!(f, "invalid {kind:?} signature: {reason}")
            }
            Self::CertificateRejected(reason) => {
                write!(f, "certified-body request QC was rejected: {reason}")
            }
            Self::ZeroCapacity => {
                f.write_str("outstanding certified-body request capacity must be non-zero")
            }
            Self::DuplicateRequest(hash) => {
                write!(f, "certified-body request {hash} is already outstanding")
            }
            Self::ConflictingRequest { existing, incoming } => write!(
                f,
                "certified-body request {incoming} conflicts with outstanding request {existing}"
            ),
            Self::CapacityExceeded { capacity } => write!(
                f,
                "outstanding certified-body request capacity {capacity} is exhausted"
            ),
            Self::UnsolicitedResponse(hash) => write!(
                f,
                "certified-body response for request {hash} is unsolicited or replayed"
            ),
            Self::DuplicateCommitCertificateRequest(hash) => write!(
                f,
                "commit-certificate request {hash} is already outstanding"
            ),
            Self::ConflictingCommitCertificateRequest { existing, incoming } => write!(
                f,
                "commit-certificate request {incoming} conflicts with outstanding request {existing}"
            ),
            Self::CommitCertificateCapacityExceeded { capacity } => write!(
                f,
                "outstanding commit-certificate request capacity {capacity} is exhausted"
            ),
            Self::UnsolicitedCommitCertificateResponse(hash) => write!(
                f,
                "commit-certificate response for request {hash} is unsolicited or replayed"
            ),
        }
    }
}

impl std::error::Error for V2TransportError {}

impl From<wire::ValidationError> for V2TransportError {
    fn from(error: wire::ValidationError) -> Self {
        Self::Wire(error)
    }
}

/// Payload chunk admitted through structural, identity, and signature checks.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct AuthenticatedPayloadChunk {
    chunk: wire::PayloadChunk,
}

impl AuthenticatedPayloadChunk {
    /// Borrow the authenticated chunk.
    pub(crate) const fn chunk(&self) -> &wire::PayloadChunk {
        &self.chunk
    }

    /// Consume the token and recover the authenticated chunk.
    pub(crate) fn into_inner(self) -> wire::PayloadChunk {
        self.chunk
    }
}

/// Certified-body request admitted through structural, identity, signature,
/// and quorum-certificate checks.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct AuthenticatedCertifiedBodyRequest {
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    request: wire::CertifiedBodyRequest,
}

impl AuthenticatedCertifiedBodyRequest {
    /// Hash of the exact signed request.
    pub(crate) const fn request_hash(&self) -> HashOf<wire::CertifiedBodyRequest> {
        self.request_hash
    }

    /// Borrow the authenticated request.
    pub(crate) const fn request(&self) -> &wire::CertifiedBodyRequest {
        &self.request
    }
}

/// Certified-body response admitted for one outstanding exact request.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct AuthenticatedCertifiedBodyResponse {
    response: wire::CertifiedBodyResponse,
}

impl AuthenticatedCertifiedBodyResponse {
    /// Borrow the authenticated response.
    pub(crate) const fn response(&self) -> &wire::CertifiedBodyResponse {
        &self.response
    }

    /// Consume the token and recover the authenticated response.
    pub(crate) fn into_inner(self) -> wire::CertifiedBodyResponse {
        self.response
    }
}

/// Commit-certificate request admitted through structural, outer-identity,
/// and requester-signature checks against one exact historical context.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct AuthenticatedCommitCertificateRequest {
    request_hash: HashOf<wire::CommitCertificateRequest>,
    request: wire::CommitCertificateRequest,
}

impl AuthenticatedCommitCertificateRequest {
    /// Hash of the exact signed request.
    pub(crate) const fn request_hash(&self) -> HashOf<wire::CommitCertificateRequest> {
        self.request_hash
    }

    /// Borrow the authenticated request.
    pub(crate) const fn request(&self) -> &wire::CommitCertificateRequest {
        &self.request
    }
}

/// Commit-certificate response authenticated for one outstanding exact request.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct AuthenticatedCommitCertificateResponse {
    request_hash: HashOf<wire::CommitCertificateRequest>,
    response: wire::CommitCertificateResponse,
}

impl AuthenticatedCommitCertificateResponse {
    /// Hash of the outstanding request authenticated by this response.
    pub(crate) const fn request_hash(&self) -> HashOf<wire::CommitCertificateRequest> {
        self.request_hash
    }

    /// Borrow the authenticated response.
    pub(crate) const fn response(&self) -> &wire::CommitCertificateResponse {
        &self.response
    }

    /// Consume the token and recover the response.
    pub(crate) fn into_inner(self) -> wire::CommitCertificateResponse {
        self.response
    }
}

/// Authenticate a payload chunk against one exact manifest and outer peer.
///
/// # Errors
///
/// Returns an error when structural commitments, the declared roster sender,
/// the authenticated transport peer, or the sender signature do not match.
pub(crate) fn authenticate_payload_chunk(
    context: &wire::HeightContext,
    manifest: &wire::PayloadManifest,
    chunk: wire::PayloadChunk,
    authenticated_sender: &PeerId,
) -> Result<AuthenticatedPayloadChunk, V2TransportError> {
    chunk.validate(context, manifest)?;
    let claimed_sender = roster_peer(context, chunk.sender)?;
    bind_outer_identity(
        TransportIdentityKind::ChunkSender,
        claimed_sender,
        authenticated_sender,
    )?;
    let preimage = chunk.signature_preimage(context, manifest)?;
    verify_signature(
        TransportSignatureKind::PayloadChunk,
        claimed_sender,
        &chunk.signature,
        &preimage,
    )?;
    Ok(AuthenticatedPayloadChunk { chunk })
}

/// Authenticate a certified-body request and its quorum certificate.
///
/// The supplied callback is the production certificate verifier; structural QC
/// validation alone is not treated as authentication.
///
/// # Errors
///
/// Returns an error when the request is malformed, its requester is not the
/// authenticated outer peer, its signature is invalid, or `verify_qc` rejects
/// the carried certificate.
pub(crate) fn authenticate_certified_body_request<VerifyQc, VerifyError>(
    context: &wire::HeightContext,
    request: wire::CertifiedBodyRequest,
    authenticated_requester: &PeerId,
    verify_qc: VerifyQc,
) -> Result<AuthenticatedCertifiedBodyRequest, V2TransportError>
where
    VerifyQc: FnOnce(&wire::HeightContext, &wire::QuorumCertificate) -> Result<(), VerifyError>,
    VerifyError: fmt::Display,
{
    request.validate(context)?;
    authenticate_certified_body_request_identity(&request, authenticated_requester)?;
    verify_qc(context, &request.certificate)
        .map_err(|error| V2TransportError::CertificateRejected(error.to_string()))?;
    Ok(AuthenticatedCertifiedBodyRequest {
        request_hash: HashOf::new(&request),
        request,
    })
}

/// Authenticate a certified-body requester before loading historical context
/// or canonical body state from disk.
pub(crate) fn authenticate_certified_body_request_identity(
    request: &wire::CertifiedBodyRequest,
    authenticated_requester: &PeerId,
) -> Result<(), V2TransportError> {
    bind_outer_identity(
        TransportIdentityKind::Requester,
        &request.requester,
        authenticated_requester,
    )?;
    verify_signature(
        TransportSignatureKind::CertifiedBodyRequest,
        &request.requester,
        &request.signature,
        &request.signature_preimage(),
    )?;
    Ok(())
}

/// Authenticate a commit-certificate request against the exact historical
/// context named by the requester.
///
/// The serving node obtains `context` from its immutable Kura finality
/// artifact, not from its current consensus height. This is what keeps catch-up
/// possible after epoch/key rotation without accepting cross-chain requests.
pub(crate) fn authenticate_commit_certificate_request(
    context: &wire::HeightContext,
    request: wire::CommitCertificateRequest,
    authenticated_requester: &PeerId,
) -> Result<AuthenticatedCommitCertificateRequest, V2TransportError> {
    request.validate(context)?;
    authenticate_commit_certificate_request_identity(&request, authenticated_requester)?;
    Ok(AuthenticatedCommitCertificateRequest {
        request_hash: HashOf::new(&request),
        request,
    })
}

/// Authenticate the requester identity before a serving node performs a Kura
/// lookup for the historical context.
///
/// Full chain/context validation still follows after the immutable finality
/// artifact is loaded. Splitting the checks prevents spoofed traffic from
/// causing disk reads while retaining context-derived replay protection.
pub(crate) fn authenticate_commit_certificate_request_identity(
    request: &wire::CommitCertificateRequest,
    authenticated_requester: &PeerId,
) -> Result<(), V2TransportError> {
    if request.protocol_version != wire::PROTOCOL_VERSION {
        return Err(wire::ValidationError::UnsupportedProtocolVersion {
            expected: wire::PROTOCOL_VERSION,
            actual: request.protocol_version,
        }
        .into());
    }
    bind_outer_identity(
        TransportIdentityKind::CommitCertificateRequester,
        &request.requester,
        authenticated_requester,
    )?;
    verify_signature(
        TransportSignatureKind::CommitCertificateRequest,
        &request.requester,
        &request.signature,
        &request.signature_preimage(),
    )?;
    Ok(())
}

/// Bounded set of exact certified-body requests awaiting a response.
///
/// A successful response atomically consumes its request. Every rejected
/// response leaves the request outstanding so a Byzantine or corrupt sender
/// cannot suppress a later valid answer.
pub(crate) struct OutstandingCertifiedBodyRequests {
    capacity: usize,
    requests: BTreeMap<HashOf<wire::CertifiedBodyRequest>, AuthenticatedCertifiedBodyRequest>,
    identities: BTreeMap<RequestIdentity, HashOf<wire::CertifiedBodyRequest>>,
}

impl OutstandingCertifiedBodyRequests {
    /// Construct an empty tracker with an explicit non-zero capacity.
    ///
    /// # Errors
    ///
    /// Returns [`V2TransportError::ZeroCapacity`] for a zero bound.
    pub(crate) fn new(capacity: usize) -> Result<Self, V2TransportError> {
        if capacity == 0 {
            return Err(V2TransportError::ZeroCapacity);
        }
        Ok(Self {
            capacity,
            requests: BTreeMap::new(),
            identities: BTreeMap::new(),
        })
    }

    /// Number of currently outstanding exact requests.
    pub(crate) fn len(&self) -> usize {
        self.requests.len()
    }

    /// Whether the tracker has no outstanding requests.
    pub(crate) fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Whether one exact request hash is currently outstanding.
    pub(crate) fn contains(&self, hash: HashOf<wire::CertifiedBodyRequest>) -> bool {
        self.requests.contains_key(&hash)
    }

    /// Register an authenticated request without eviction.
    ///
    /// Exact repeats and logically conflicting reissues are distinguished from
    /// capacity exhaustion and never mutate the tracker.
    ///
    /// # Errors
    ///
    /// Returns a duplicate, conflict, or capacity error as applicable.
    pub(crate) fn register(
        &mut self,
        authenticated: AuthenticatedCertifiedBodyRequest,
    ) -> Result<(), V2TransportError> {
        let incoming = authenticated.request_hash;
        if self.requests.contains_key(&incoming) {
            return Err(V2TransportError::DuplicateRequest(incoming));
        }
        let identity = RequestIdentity::from(&authenticated.request);
        if let Some(existing) = self.identities.get(&identity) {
            return Err(V2TransportError::ConflictingRequest {
                existing: *existing,
                incoming,
            });
        }
        if self.requests.len() >= self.capacity {
            return Err(V2TransportError::CapacityExceeded {
                capacity: self.capacity,
            });
        }
        self.requests.insert(incoming, authenticated);
        self.identities.insert(identity, incoming);
        Ok(())
    }

    /// Cancel one exact outstanding request and release its logical identity.
    ///
    /// View changes and locally recovered bodies can make a fetch unnecessary.
    /// Removing both indexes prevents abandoned requests from permanently
    /// consuming the bounded request capacity while late responses remain
    /// correctly classified as unsolicited.
    pub(crate) fn cancel(&mut self, request_hash: HashOf<wire::CertifiedBodyRequest>) -> bool {
        let Some(authenticated) = self.requests.remove(&request_hash) else {
            return false;
        };
        let identity = RequestIdentity::from(authenticated.request());
        let removed = self.identities.remove(&identity);
        debug_assert_eq!(removed, Some(request_hash));
        true
    }

    /// Authenticate and consume a response for an outstanding exact request.
    ///
    /// # Errors
    ///
    /// Returns an error for unsolicited/replayed responses, malformed bodies
    /// or manifests, uncertified/spoofed responders, and invalid signatures.
    /// The outstanding request is retained on every error.
    pub(crate) fn authenticate_response(
        &mut self,
        context: &wire::HeightContext,
        response: wire::CertifiedBodyResponse,
        authenticated_responder: &PeerId,
    ) -> Result<AuthenticatedCertifiedBodyResponse, V2TransportError> {
        let request_hash = response.request_hash;
        let identity = {
            let authenticated_request = self
                .requests
                .get(&request_hash)
                .ok_or(V2TransportError::UnsolicitedResponse(request_hash))?;
            let request = authenticated_request.request();
            let claimed_responder = roster_peer(context, response.responder)?;
            bind_outer_identity(
                TransportIdentityKind::Responder,
                claimed_responder,
                authenticated_responder,
            )?;
            if request
                .certificate
                .signers
                .binary_search(&response.responder)
                .is_err()
            {
                return Err(wire::ValidationError::ResponderNotCertified.into());
            }
            response.validate_against(context, request, authenticated_responder)?;
            verify_signature(
                TransportSignatureKind::CertifiedBodyResponse,
                claimed_responder,
                &response.signature,
                &response.signature_preimage(),
            )?;
            RequestIdentity::from(request)
        };

        let removed = self.requests.remove(&request_hash);
        debug_assert!(removed.is_some(), "validated request remains registered");
        let removed_identity = self.identities.remove(&identity);
        debug_assert_eq!(removed_identity, Some(request_hash));
        Ok(AuthenticatedCertifiedBodyResponse { response })
    }
}

/// Bounded exact-request tracker for CommitQC discovery.
///
/// Responses are authenticated without consuming their request. The serialized
/// caller removes the request only after the returned CommitQC is successfully
/// admitted to the authoritative reducer queue, so queue backpressure cannot
/// turn a valid response into a permanent catch-up stall.
pub(crate) struct OutstandingCommitCertificateRequests {
    capacity: usize,
    requests:
        BTreeMap<HashOf<wire::CommitCertificateRequest>, AuthenticatedCommitCertificateRequest>,
    identities: BTreeMap<CommitCertificateRequestIdentity, HashOf<wire::CommitCertificateRequest>>,
}

impl OutstandingCommitCertificateRequests {
    /// Construct an empty bounded tracker.
    pub(crate) fn new(capacity: usize) -> Result<Self, V2TransportError> {
        if capacity == 0 {
            return Err(V2TransportError::ZeroCapacity);
        }
        Ok(Self {
            capacity,
            requests: BTreeMap::new(),
            identities: BTreeMap::new(),
        })
    }

    /// Number of outstanding requests.
    pub(crate) fn len(&self) -> usize {
        self.requests.len()
    }

    /// Whether no CommitQC discovery request remains outstanding.
    pub(crate) fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Whether the exact request remains outstanding.
    pub(crate) fn contains(&self, request_hash: HashOf<wire::CommitCertificateRequest>) -> bool {
        self.requests.contains_key(&request_hash)
    }

    /// Borrow one exact outstanding request for deterministic retransmission.
    pub(crate) fn request(
        &self,
        request_hash: HashOf<wire::CommitCertificateRequest>,
    ) -> Option<&wire::CommitCertificateRequest> {
        self.requests
            .get(&request_hash)
            .map(AuthenticatedCommitCertificateRequest::request)
    }

    /// Register one fully authenticated signed request without eviction.
    pub(crate) fn register(
        &mut self,
        authenticated: AuthenticatedCommitCertificateRequest,
    ) -> Result<(), V2TransportError> {
        let incoming = authenticated.request_hash;
        if self.requests.contains_key(&incoming) {
            return Err(V2TransportError::DuplicateCommitCertificateRequest(
                incoming,
            ));
        }
        let identity = CommitCertificateRequestIdentity::from(authenticated.request());
        if let Some(existing) = self.identities.get(&identity) {
            return Err(V2TransportError::ConflictingCommitCertificateRequest {
                existing: *existing,
                incoming,
            });
        }
        if self.requests.len() >= self.capacity {
            return Err(V2TransportError::CommitCertificateCapacityExceeded {
                capacity: self.capacity,
            });
        }
        self.requests.insert(incoming, authenticated);
        self.identities.insert(identity, incoming);
        Ok(())
    }

    /// Authenticate a response without consuming the outstanding request.
    ///
    /// Aggregate CommitQC authentication deliberately remains in the ordinary
    /// consensus ingress used by [`wire::ConsensusMessageV2Payload::QuorumCertificate`].
    /// This transport check authenticates only the exact request/response and
    /// current responder identity; the request is not consumed here.
    pub(crate) fn authenticate_response(
        &self,
        context: &wire::HeightContext,
        response: wire::CommitCertificateResponse,
        authenticated_responder: &PeerId,
    ) -> Result<AuthenticatedCommitCertificateResponse, V2TransportError> {
        let request_hash = response.request_hash;
        let authenticated_request = self.requests.get(&request_hash).ok_or(
            V2TransportError::UnsolicitedCommitCertificateResponse(request_hash),
        )?;
        response.validate_against(context, authenticated_request.request())?;
        bind_outer_identity(
            TransportIdentityKind::CommitCertificateResponder,
            &response.responder,
            authenticated_responder,
        )?;
        verify_signature(
            TransportSignatureKind::CommitCertificateResponse,
            &response.responder,
            &response.signature,
            &response.signature_preimage(),
        )?;
        Ok(AuthenticatedCommitCertificateResponse {
            request_hash,
            response,
        })
    }

    /// Consume a request only after its certificate entered the reducer queue.
    pub(crate) fn complete(
        &mut self,
        request_hash: HashOf<wire::CommitCertificateRequest>,
    ) -> bool {
        let Some(authenticated) = self.requests.remove(&request_hash) else {
            return false;
        };
        let identity = CommitCertificateRequestIdentity::from(authenticated.request());
        let removed = self.identities.remove(&identity);
        debug_assert_eq!(removed, Some(request_hash));
        true
    }

    /// Cancel obsolete work when the active height changes by another path.
    pub(crate) fn cancel(&mut self, request_hash: HashOf<wire::CommitCertificateRequest>) -> bool {
        self.complete(request_hash)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CommitCertificateRequestIdentity {
    protocol_version: u16,
    context_id: wire::HeightContextId,
    height: wire::Height,
    requester: PeerId,
}

impl From<&wire::CommitCertificateRequest> for CommitCertificateRequestIdentity {
    fn from(request: &wire::CommitCertificateRequest) -> Self {
        Self {
            protocol_version: request.protocol_version,
            context_id: request.context_id,
            height: request.height,
            requester: request.requester.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RequestIdentity {
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    requester: PeerId,
}

impl From<&wire::CertifiedBodyRequest> for RequestIdentity {
    fn from(request: &wire::CertifiedBodyRequest) -> Self {
        Self {
            round: request.round,
            subject: request.subject,
            requester: request.requester.clone(),
        }
    }
}

fn roster_peer(
    context: &wire::HeightContext,
    index: wire::ValidatorIndex,
) -> Result<&PeerId, V2TransportError> {
    let index = usize::try_from(index).map_err(|_| wire::ValidationError::SignerOutOfRange)?;
    context
        .roster
        .get(index)
        .map(|entry| &entry.validator)
        .ok_or_else(|| wire::ValidationError::SignerOutOfRange.into())
}

fn bind_outer_identity(
    kind: TransportIdentityKind,
    claimed: &PeerId,
    authenticated: &PeerId,
) -> Result<(), V2TransportError> {
    if claimed != authenticated {
        return Err(V2TransportError::OuterIdentityMismatch {
            kind,
            claimed: claimed.clone(),
            authenticated: authenticated.clone(),
        });
    }
    Ok(())
}

fn verify_signature(
    kind: TransportSignatureKind,
    signer: &PeerId,
    signature: &[u8],
    preimage: &[u8],
) -> Result<(), V2TransportError> {
    let signature = Signature::try_from_bytes(signature).map_err(|error| {
        V2TransportError::InvalidSignature {
            kind,
            reason: error.to_string(),
        }
    })?;
    signature
        .verify(signer.public_key(), preimage)
        .map_err(|error| V2TransportError::InvalidSignature {
            kind,
            reason: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair};

    use super::*;

    struct Fixture {
        context: wire::HeightContext,
        validators: Vec<KeyPair>,
        observer: KeyPair,
        body: Vec<u8>,
        manifest: wire::PayloadManifest,
    }

    impl Fixture {
        fn new() -> Self {
            let mut validators = (1_u8..=4)
                .map(|seed| {
                    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                        .expect("deterministic validator key")
                })
                .collect::<Vec<_>>();
            validators.sort_by(|left, right| left.public_key().cmp(right.public_key()));
            let roster = validators
                .iter()
                .map(|key| wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                })
                .collect::<Vec<_>>();
            let context = wire::HeightContext {
                chain_id: "sumeragi-v2-transport-test".into(),
                protocol_version: wire::PROTOCOL_VERSION,
                height: 1,
                epoch: 7,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"transport-test-nexus-amx-context"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::Plain,
                    chunk_size_bytes: 64,
                    data_shards: 0,
                    parity_shards: 0,
                    max_payload_size_bytes: 4096,
                    max_chunk_count: 64,
                },
                leader_seed: [0x47; 32],
            };
            let body = b"certified transport body".to_vec();
            let round = wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 2,
            };
            let subject = wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new(b"transport block")),
                payload_hash: Hash::new(&body),
            };
            let manifest = wire::PayloadManifest::derive(
                &context,
                round,
                subject,
                u64::try_from(body.len()).expect("small body"),
                std::slice::from_ref(&body),
            )
            .expect("canonical fixture manifest");
            let observer = KeyPair::try_from_seed(vec![90; 32], Algorithm::Ed25519)
                .expect("deterministic observer key");
            Self {
                context,
                validators,
                observer,
                body,
                manifest,
            }
        }

        fn peer(key: &KeyPair) -> PeerId {
            PeerId::new(key.public_key().clone())
        }

        fn signed_chunk(&self, sender: wire::ValidatorIndex) -> wire::PayloadChunk {
            let mut chunk = wire::PayloadChunk {
                manifest_hash: HashOf::new(&self.manifest),
                index: 0,
                bytes: self.body.clone(),
                sender,
                signature: Vec::new(),
            };
            let preimage = chunk
                .signature_preimage(&self.context, &self.manifest)
                .expect("chunk preimage");
            let signer = &self.validators[usize::try_from(sender).expect("small index")];
            chunk.signature = Signature::new(signer.private_key(), &preimage)
                .payload()
                .to_vec();
            chunk
        }

        fn signed_request(&self) -> wire::CertifiedBodyRequest {
            let mut request = wire::CertifiedBodyRequest {
                round: self.manifest.round,
                subject: self.manifest.subject,
                certificate: wire::QuorumCertificate {
                    round: self.manifest.round,
                    phase: wire::GlobalPhase::Prepare,
                    subject: self.manifest.subject,
                    signers: vec![0, 1, 2],
                    aggregate_signature: vec![0xA5; 48],
                },
                requester: Self::peer(&self.observer),
                signature: Vec::new(),
            };
            request.signature =
                Signature::new(self.observer.private_key(), &request.signature_preimage())
                    .payload()
                    .to_vec();
            request
        }

        fn signed_response(
            &self,
            request: &wire::CertifiedBodyRequest,
            responder: wire::ValidatorIndex,
        ) -> wire::CertifiedBodyResponse {
            let mut response = wire::CertifiedBodyResponse {
                request_hash: HashOf::new(request),
                manifest: self.manifest.clone(),
                body: self.body.clone(),
                responder,
                signature: Vec::new(),
            };
            let signer = &self.validators[usize::try_from(responder).expect("small index")];
            response.signature =
                Signature::new(signer.private_key(), &response.signature_preimage())
                    .payload()
                    .to_vec();
            response
        }

        fn authenticate_request(
            &self,
            request: wire::CertifiedBodyRequest,
        ) -> Result<AuthenticatedCertifiedBodyRequest, V2TransportError> {
            authenticate_certified_body_request(
                &self.context,
                request,
                &Self::peer(&self.observer),
                |_, _| Ok::<(), &'static str>(()),
            )
        }
    }

    #[test]
    fn payload_chunk_binds_exact_manifest_outer_sender_and_signature() {
        let fixture = Fixture::new();
        let chunk = fixture.signed_chunk(0);
        let sender = Fixture::peer(&fixture.validators[0]);
        let authenticated =
            authenticate_payload_chunk(&fixture.context, &fixture.manifest, chunk.clone(), &sender)
                .expect("valid chunk");
        assert_eq!(authenticated.chunk(), &chunk);

        let spoof = Fixture::peer(&fixture.validators[1]);
        assert!(matches!(
            authenticate_payload_chunk(&fixture.context, &fixture.manifest, chunk.clone(), &spoof),
            Err(V2TransportError::OuterIdentityMismatch {
                kind: TransportIdentityKind::ChunkSender,
                ..
            })
        ));

        let mut wrong_signature = chunk.clone();
        wrong_signature.signature = Signature::new(
            fixture.validators[1].private_key(),
            &wrong_signature
                .signature_preimage(&fixture.context, &fixture.manifest)
                .expect("preimage"),
        )
        .payload()
        .to_vec();
        assert!(matches!(
            authenticate_payload_chunk(
                &fixture.context,
                &fixture.manifest,
                wrong_signature,
                &sender
            ),
            Err(V2TransportError::InvalidSignature {
                kind: TransportSignatureKind::PayloadChunk,
                ..
            })
        ));

        let mut other_manifest = fixture.manifest.clone();
        other_manifest.subject.block_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"other transport block"));
        assert!(matches!(
            authenticate_payload_chunk(&fixture.context, &other_manifest, chunk, &sender),
            Err(V2TransportError::Wire(
                wire::ValidationError::ManifestHashMismatch
            ))
        ));
    }

    #[test]
    fn request_binds_outer_requester_signature_and_qc_verifier() {
        let fixture = Fixture::new();
        let request = fixture.signed_request();
        let observer = Fixture::peer(&fixture.observer);
        let mut verifier_called = false;
        let authenticated = authenticate_certified_body_request(
            &fixture.context,
            request.clone(),
            &observer,
            |context, certificate| {
                verifier_called = true;
                assert_eq!(context.id(), fixture.context.id());
                assert_eq!(certificate, &request.certificate);
                Ok::<(), &'static str>(())
            },
        )
        .expect("valid request");
        assert!(verifier_called);
        assert_eq!(authenticated.request(), &request);

        let spoof = Fixture::peer(&fixture.validators[0]);
        assert!(matches!(
            authenticate_certified_body_request(
                &fixture.context,
                request.clone(),
                &spoof,
                |_, _| -> Result<(), &'static str> {
                    panic!("spoofed requester must be rejected before QC verification")
                }
            ),
            Err(V2TransportError::OuterIdentityMismatch {
                kind: TransportIdentityKind::Requester,
                ..
            })
        ));

        let mut wrong_signature = request.clone();
        wrong_signature.signature = Signature::new(
            fixture.validators[0].private_key(),
            &wrong_signature.signature_preimage(),
        )
        .payload()
        .to_vec();
        assert!(matches!(
            authenticate_certified_body_request(
                &fixture.context,
                wrong_signature,
                &observer,
                |_, _| -> Result<(), &'static str> {
                    panic!("bad request signature must be rejected before QC verification")
                }
            ),
            Err(V2TransportError::InvalidSignature {
                kind: TransportSignatureKind::CertifiedBodyRequest,
                ..
            })
        ));

        assert_eq!(
            authenticate_certified_body_request(
                &fixture.context,
                request,
                &observer,
                |_, _| Err::<(), _>("bad aggregate")
            ),
            Err(V2TransportError::CertificateRejected(
                "bad aggregate".to_owned()
            ))
        );
    }

    #[test]
    fn invalid_responses_never_consume_the_outstanding_request() {
        let fixture = Fixture::new();
        let request = fixture.signed_request();
        let request_hash = HashOf::new(&request);
        let mut tracker = OutstandingCertifiedBodyRequests::new(2).expect("positive capacity");
        tracker
            .register(
                fixture
                    .authenticate_request(request.clone())
                    .expect("request"),
            )
            .expect("register request");

        let valid = fixture.signed_response(&request, 0);
        let valid_sender = Fixture::peer(&fixture.validators[0]);
        let spoof = Fixture::peer(&fixture.validators[1]);
        assert!(matches!(
            tracker.authenticate_response(&fixture.context, valid.clone(), &spoof),
            Err(V2TransportError::OuterIdentityMismatch {
                kind: TransportIdentityKind::Responder,
                ..
            })
        ));
        assert!(tracker.contains(request_hash));

        let mut wrong_request = valid.clone();
        wrong_request.request_hash = HashOf::from_untyped_unchecked(Hash::new(b"not outstanding"));
        assert!(matches!(
            tracker.authenticate_response(&fixture.context, wrong_request, &valid_sender),
            Err(V2TransportError::UnsolicitedResponse(_))
        ));
        assert!(tracker.contains(request_hash));

        let mut wrong_signature = valid.clone();
        wrong_signature.signature = Signature::new(
            fixture.validators[1].private_key(),
            &wrong_signature.signature_preimage(),
        )
        .payload()
        .to_vec();
        assert!(matches!(
            tracker.authenticate_response(&fixture.context, wrong_signature, &valid_sender),
            Err(V2TransportError::InvalidSignature {
                kind: TransportSignatureKind::CertifiedBodyResponse,
                ..
            })
        ));
        assert!(tracker.contains(request_hash));

        let mut tampered = valid.clone();
        tampered.body.push(0);
        assert!(matches!(
            tracker.authenticate_response(&fixture.context, tampered, &valid_sender),
            Err(V2TransportError::Wire(
                wire::ValidationError::CertifiedBodyHashMismatch
            ))
        ));
        assert!(tracker.contains(request_hash));

        let mut tampered_manifest = valid.clone();
        tampered_manifest.manifest.chunk_root = Hash::new(b"tampered chunk root");
        assert!(matches!(
            tracker.authenticate_response(&fixture.context, tampered_manifest, &valid_sender),
            Err(V2TransportError::Wire(
                wire::ValidationError::ChunkRootMismatch
            ))
        ));
        assert!(tracker.contains(request_hash));

        let uncertified = fixture.signed_response(&request, 3);
        let uncertified_sender = Fixture::peer(&fixture.validators[3]);
        assert!(matches!(
            tracker.authenticate_response(&fixture.context, uncertified, &uncertified_sender),
            Err(V2TransportError::Wire(
                wire::ValidationError::ResponderNotCertified
            ))
        ));
        assert!(tracker.contains(request_hash));

        let admitted = tracker
            .authenticate_response(&fixture.context, valid.clone(), &valid_sender)
            .expect("valid certified response");
        assert_eq!(admitted.response(), &valid);
        assert!(tracker.is_empty());
        assert_eq!(
            tracker.authenticate_response(&fixture.context, valid, &valid_sender),
            Err(V2TransportError::UnsolicitedResponse(request_hash))
        );
    }

    #[test]
    fn unsolicited_response_is_rejected_without_state() {
        let fixture = Fixture::new();
        let request = fixture.signed_request();
        let response = fixture.signed_response(&request, 0);
        let sender = Fixture::peer(&fixture.validators[0]);
        let mut tracker = OutstandingCertifiedBodyRequests::new(1).expect("positive capacity");
        assert_eq!(
            tracker.authenticate_response(&fixture.context, response, &sender),
            Err(V2TransportError::UnsolicitedResponse(HashOf::new(&request)))
        );
    }

    #[test]
    fn cancellation_releases_both_capacity_and_logical_identity() {
        let fixture = Fixture::new();
        let request = fixture.signed_request();
        let request_hash = HashOf::new(&request);
        let authenticated = fixture
            .authenticate_request(request.clone())
            .expect("authenticate request");
        let mut tracker = OutstandingCertifiedBodyRequests::new(1).expect("positive capacity");
        tracker.register(authenticated).expect("register request");

        assert!(tracker.cancel(request_hash));
        assert!(tracker.is_empty());
        assert!(!tracker.cancel(request_hash));
        tracker
            .register(
                fixture
                    .authenticate_request(request)
                    .expect("authenticate reissued request"),
            )
            .expect("cancelled logical identity can be reissued");
    }

    #[test]
    fn tracker_distinguishes_duplicates_conflicts_and_capacity() {
        assert!(matches!(
            OutstandingCertifiedBodyRequests::new(0),
            Err(V2TransportError::ZeroCapacity)
        ));

        let fixture = Fixture::new();
        let request = fixture.signed_request();
        let request_hash = HashOf::new(&request);
        let mut tracker = OutstandingCertifiedBodyRequests::new(1).expect("positive capacity");
        tracker
            .register(
                fixture
                    .authenticate_request(request.clone())
                    .expect("request"),
            )
            .expect("register request");
        assert_eq!(tracker.len(), 1);
        assert_eq!(
            tracker.register(
                fixture
                    .authenticate_request(request.clone())
                    .expect("duplicate")
            ),
            Err(V2TransportError::DuplicateRequest(request_hash))
        );

        let mut conflicting = request.clone();
        conflicting.certificate.aggregate_signature = vec![0x5A; 48];
        conflicting.signature = Signature::new(
            fixture.observer.private_key(),
            &conflicting.signature_preimage(),
        )
        .payload()
        .to_vec();
        let conflicting_hash = HashOf::new(&conflicting);
        assert_eq!(
            tracker.register(
                fixture
                    .authenticate_request(conflicting)
                    .expect("authenticated conflict")
            ),
            Err(V2TransportError::ConflictingRequest {
                existing: request_hash,
                incoming: conflicting_hash,
            })
        );

        let mut second = request;
        second.round.view += 1;
        second.certificate.round = second.round;
        second.signature =
            Signature::new(fixture.observer.private_key(), &second.signature_preimage())
                .payload()
                .to_vec();
        assert_eq!(
            tracker.register(
                fixture
                    .authenticate_request(second)
                    .expect("authenticated second request")
            ),
            Err(V2TransportError::CapacityExceeded { capacity: 1 })
        );
        assert_eq!(tracker.len(), 1);
    }
}
