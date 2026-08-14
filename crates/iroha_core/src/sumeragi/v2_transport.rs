//! Authenticated transport boundary for Sumeragi v2 payload dissemination.
//!
//! The consensus reducer accepts only already-authenticated control inputs. This
//! module applies the same rule to payload chunks and certified body fetches:
//! structural wire validation, transport identity binding, and cryptographic
//! authentication all complete before an adapter may act on the payload.
use core::fmt;
use std::collections::{BTreeMap, btree_map::Entry};
#[cfg(test)]
use std::collections::BTreeSet;
use iroha_crypto::{HashOf, Signature};
use iroha_data_model::{
    block::{consensus_v2 as wire, decode_framed_signed_block},
    peer::PeerId,
};
use super::v2::{SumeragiV2Adapter, VerifiedHeightContext, verify_historical_quorum_certificate};
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
    /// The fixed quorum-certificate verifier rejected a request.
    CertificateRejected(String),
    /// A certified body was not a canonical resultless proposal block.
    InvalidProposalBody(String),
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
    /// The two exact-request indexes disagree for an internally owned request.
    InconsistentRequestIndex(HashOf<wire::CertifiedBodyRequest>),
    /// Another fully authenticated response already owns this request family.
    ConflictingCertifiedBodyResponseClaim {
        /// Outstanding request whose one volatile response slot is occupied.
        request: HashOf<wire::CertifiedBodyRequest>,
        /// Exact response already owning the slot.
        claimed: HashOf<wire::CertifiedBodyResponse>,
        /// Different authenticated response which attempted to replace it.
        incoming: HashOf<wire::CertifiedBodyResponse>,
    },
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
            Self::InvalidProposalBody(reason) => {
                write!(f, "invalid certified Sumeragi v2 proposal body: {reason}")
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
            Self::InconsistentRequestIndex(hash) => write!(
                f,
                "certified-body request {hash} has inconsistent exact ownership indexes"
            ),
            Self::ConflictingCertifiedBodyResponseClaim {
                request,
                claimed,
                incoming,
            } => write!(
                f,
                "certified-body response {incoming} conflicts with claimed response {claimed} for request {request}"
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
    /// Authenticate one response against this exact already-authenticated request.
    ///
    /// This request-scoped entry point lets a dedicated lifecycle owner retain
    /// request authority without inserting it into the ordinary outstanding
    /// tracker. The authenticated response constructor remains private here.
    pub(in crate::sumeragi) fn authenticate_response(
        &self,
        context: &wire::HeightContext,
        response: wire::CertifiedBodyResponse,
        authenticated_responder: &PeerId,
    ) -> Result<AuthenticatedCertifiedBodyResponse, V2TransportError> {
        authenticate_certified_body_response_for_request(
            context,
            self,
            response,
            authenticated_responder,
        )
    }
}
fn authenticate_certified_body_response_for_request(
    context: &wire::HeightContext,
    authenticated_request: &AuthenticatedCertifiedBodyRequest,
    response: wire::CertifiedBodyResponse,
    authenticated_responder: &PeerId,
) -> Result<AuthenticatedCertifiedBodyResponse, V2TransportError> {
    let claimed_responder = roster_peer(context, response.responder)?;
    bind_outer_identity(
        TransportIdentityKind::Responder,
        claimed_responder,
        authenticated_responder,
    )?;
    response.validate_against(
        context,
        authenticated_request.request(),
        authenticated_responder,
    )?;
    verify_signature(
        TransportSignatureKind::CertifiedBodyResponse,
        claimed_responder,
        &response.signature,
        &response.signature_preimage(),
    )?;
    let proposal = decode_framed_signed_block(&response.body)
        .map_err(|error| V2TransportError::InvalidProposalBody(error.to_string()))?;
    if !proposal.is_resultless_proposal() {
        return Err(V2TransportError::InvalidProposalBody(
            "execution results or result root are present".to_owned(),
        ));
    }
    Ok(AuthenticatedCertifiedBodyResponse { response })
}
/// Certified-body response admitted for one outstanding exact request.
#[derive(Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct AuthenticatedCertifiedBodyResponse {
    response: wire::CertifiedBodyResponse,
}
impl AuthenticatedCertifiedBodyResponse {
    /// Borrow the authenticated response.
    pub(crate) const fn response(&self) -> &wire::CertifiedBodyResponse {
        &self.response
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
/// Authenticate a certified-body request against the live adapter's frozen
/// roster authority.
///
/// Unlike the test-only callback helper below, this production entry point
/// cannot substitute caller-selected certificate semantics: the adapter's
/// ordinary authenticated-ingress verifier must accept the carried QC.
///
/// # Errors
///
/// Returns an error when the request is malformed, its requester is not the
/// authenticated outer peer, its signature is invalid, or the live adapter
/// rejects the carried certificate.
pub(crate) fn authenticate_certified_body_request_with_live_adapter(
    context: &wire::HeightContext,
    request: wire::CertifiedBodyRequest,
    authenticated_requester: &PeerId,
    adapter: &SumeragiV2Adapter,
) -> Result<AuthenticatedCertifiedBodyRequest, V2TransportError> {
    validate_certified_body_request(context, &request, authenticated_requester)?;
    adapter
        .authenticate(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(request.certificate.clone()),
        ))
        .map_err(|error| V2TransportError::CertificateRejected(error.to_string()))?;
    Ok(authenticated_certified_body_request(request))
}
/// Authenticate a certified-body request against an immutable historical
/// roster and its exact BLS proofs of possession.
///
/// The verifier is fixed by this function; callers can supply authority bytes
/// but cannot replace cryptographic verification with an arbitrary callback.
///
/// # Errors
///
/// Returns an error when request authentication fails or the fixed historical
/// QC verifier rejects the carried certificate.
pub(crate) fn authenticate_certified_body_request_with_validator_pops(
    context: &wire::HeightContext,
    proofs_of_possession: &[Vec<u8>],
    request: wire::CertifiedBodyRequest,
    authenticated_requester: &PeerId,
) -> Result<AuthenticatedCertifiedBodyRequest, V2TransportError> {
    validate_certified_body_request(context, &request, authenticated_requester)?;
    verify_historical_quorum_certificate(context, proofs_of_possession, &request.certificate)
        .map_err(|error| V2TransportError::CertificateRejected(error.to_string()))?;
    Ok(authenticated_certified_body_request(request))
}
/// Authenticate a certified-body request against one already verified height.
///
/// # Errors
///
/// Returns an error when exact request authentication or fixed QC verification
/// fails under `verified`.
pub(crate) fn authenticate_certified_body_request_with_verified_height(
    verified: &VerifiedHeightContext,
    request: wire::CertifiedBodyRequest,
    authenticated_requester: &PeerId,
) -> Result<AuthenticatedCertifiedBodyRequest, V2TransportError> {
    authenticate_certified_body_request_with_validator_pops(
        verified.context(),
        verified.proofs_of_possession(),
        request,
        authenticated_requester,
    )
}
/// Test-only request mint supporting deliberately synthetic certificate
/// policies. Production code must use one of the fixed verifier-backed entry
/// points above.
#[cfg(test)]
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
    validate_certified_body_request(context, &request, authenticated_requester)?;
    verify_qc(context, &request.certificate)
        .map_err(|error| V2TransportError::CertificateRejected(error.to_string()))?;
    Ok(authenticated_certified_body_request(request))
}
fn validate_certified_body_request(
    context: &wire::HeightContext,
    request: &wire::CertifiedBodyRequest,
    authenticated_requester: &PeerId,
) -> Result<(), V2TransportError> {
    request.validate(context)?;
    authenticate_certified_body_request_identity(request, authenticated_requester)
}
fn authenticated_certified_body_request(
    request: wire::CertifiedBodyRequest,
) -> AuthenticatedCertifiedBodyRequest {
    AuthenticatedCertifiedBodyRequest {
        request_hash: HashOf::new(&request),
        request,
    }
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
/// Authentication never consumes a request. The serialized executor retires
/// it only after the authenticated body has passed canonical manifest
/// reconstruction and entered the reducer queue, so a Byzantine or corrupt
/// sender cannot suppress a later valid answer.
pub(crate) struct OutstandingCertifiedBodyRequests {
    capacity: usize,
    requests: BTreeMap<HashOf<wire::CertifiedBodyRequest>, AuthenticatedCertifiedBodyRequest>,
    identities: BTreeMap<RequestIdentity, HashOf<wire::CertifiedBodyRequest>>,
    /// At most one fully authenticated physical response occurrence per
    /// outstanding request. This map shares `requests` capacity rather than
    /// introducing another environment-controlled bound.
    response_claims:
        BTreeMap<HashOf<wire::CertifiedBodyRequest>, HashOf<wire::CertifiedBodyResponse>>,
}
/// Result of acquiring the one volatile response occurrence for an exact
/// outstanding certified-body request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CertifiedBodyResponseClaimDisposition {
    /// This response acquired the request's previously empty occurrence slot.
    Acquired,
    /// The exact same authenticated response already owns the slot.
    Coalesced,
}
/// Read-only state of the one volatile response occurrence for an exact
/// outstanding certified-body request.
///
/// This preflight never acquires the slot. The serialized executor must still
/// call [`OutstandingCertifiedBodyRequests::prepare_authenticated_response_claim`]
/// after its runtime and service reservations have been planned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) enum CertifiedBodyResponseClaimPreflight {
    /// No authenticated response currently owns the request family.
    Vacant,
    /// The byte-for-byte authenticated response already owns the family.
    ExactRetransmission,
}
/// Borrow-bound response-family claim prepared before an external commit.
///
/// The exclusive tracker borrow prevents request retirement, cancellation, or
/// another response claim between the read-only preflight and the infallible
/// insertion/coalescence tail. Dropping this token performs no mutation. A
/// future composite late-response transaction may therefore hold it across
/// the exact queue CAS and call [`Self::commit`] only after that CAS succeeds.
#[must_use = "dropping a prepared response claim leaves the family unchanged"]
pub(crate) struct PreparedCertifiedBodyResponseClaim<'a> {
    tracker: &'a mut OutstandingCertifiedBodyRequests,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    preflight: CertifiedBodyResponseClaimPreflight,
}
impl PreparedCertifiedBodyResponseClaim<'_> {
    /// Return the exact read-only family state frozen by this borrow.
    #[cfg(test)]
    pub(crate) const fn preflight(&self) -> CertifiedBodyResponseClaimPreflight {
        self.preflight
    }
    /// Return the exact signed request family frozen by this token.
    #[cfg(test)]
    pub(crate) const fn request_hash(&self) -> HashOf<wire::CertifiedBodyRequest> {
        self.request_hash
    }
    /// Return the exact authenticated response occurrence frozen by this token.
    #[cfg(test)]
    pub(crate) const fn response_hash(&self) -> HashOf<wire::CertifiedBodyResponse> {
        self.response_hash
    }
    /// Commit the already validated family claim without another lookup that
    /// can reject. Any violated assertion is an internal fail-stop invariant,
    /// never a retryable post-queue error.
    pub(in crate::sumeragi) fn commit(self) -> CertifiedBodyResponseClaimDisposition {
        assert!(
            self.tracker.requests.contains_key(&self.request_hash),
            "prepared response claim retains its exact outstanding request"
        );
        match self.preflight {
            CertifiedBodyResponseClaimPreflight::ExactRetransmission => {
                assert_eq!(
                    self.tracker.response_claims.get(&self.request_hash),
                    Some(&self.response_hash),
                    "prepared retransmission retains the exact family owner"
                );
                CertifiedBodyResponseClaimDisposition::Coalesced
            }
            CertifiedBodyResponseClaimPreflight::Vacant => {
                let Entry::Vacant(slot) = self.tracker.response_claims.entry(self.request_hash)
                else {
                    panic!("exclusive prepared claim cannot overwrite a family owner")
                };
                slot.insert(self.response_hash);
                assert!(self.tracker.response_claims.len() <= self.tracker.requests.len());
                assert!(self.tracker.response_claims.len() <= self.tracker.capacity);
                CertifiedBodyResponseClaimDisposition::Acquired
            }
        }
    }
}
/// Preflighted insertion into both exact certified-request indexes.
///
/// The tracker is serialized by its executor owner. Planning performs every
/// duplicate, logical-identity, and capacity check; committing this value
/// after the body-fetch service accepts its exact task is insertion-only and
/// cannot reject.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CertifiedBodyRequestRegistrationPlan {
    incoming: HashOf<wire::CertifiedBodyRequest>,
    identity: RequestIdentity,
    authenticated: AuthenticatedCertifiedBodyRequest,
}
/// Preflighted removal from both exact certified-request indexes.
///
/// The exact logical identity is retained in the plan so commit needs no
/// fallible lookup after an external body-fetch owner has transferred its
/// completion to the executor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CertifiedBodyRequestRetirementPlan {
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    identity: RequestIdentity,
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
            response_claims: BTreeMap::new(),
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
    /// Whether the ordinary tracker already owns the logical identity of one
    /// separately authenticated request.
    ///
    /// The private [`RequestIdentity`] does not escape this module; dedicated
    /// lifecycle owners use this comparison oracle to prevent a second owner
    /// with different signature bytes from bypassing the ordinary identity
    /// fence.
    pub(in crate::sumeragi) fn contains_authenticated_identity(
        &self,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> bool {
        self.identities
            .contains_key(&RequestIdentity::from(authenticated.request()))
    }
    /// Validate the complete request, logical-identity, and response-claim cut.
    pub(crate) fn validate_exact_indexes(&self) -> bool {
        self.capacity != 0
            && self.requests.len() <= self.capacity
            && self.requests.len() == self.identities.len()
            && self.response_claims.len() <= self.requests.len()
            && self.requests.iter().all(|(request_hash, authenticated)| {
                authenticated.request_hash() == *request_hash
                    && HashOf::new(authenticated.request()) == *request_hash
                    && self
                        .identities
                        .get(&RequestIdentity::from(authenticated.request()))
                        == Some(request_hash)
            })
            && self.identities.iter().all(|(identity, request_hash)| {
                self.requests
                    .get(request_hash)
                    .is_some_and(|authenticated| {
                        RequestIdentity::from(authenticated.request()) == *identity
                    })
            })
            && self
                .response_claims
                .keys()
                .all(|request_hash| self.requests.contains_key(request_hash))
    }
    /// Exact sorted set of outstanding signed-request hashes.
    #[cfg(test)]
    pub(crate) fn hashes(&self) -> BTreeSet<HashOf<wire::CertifiedBodyRequest>> {
        self.requests.keys().copied().collect()
    }
    /// Number of volatile authenticated response occurrences currently claimed.
    #[cfg(test)]
    pub(crate) fn response_claim_count(&self) -> usize {
        self.response_claims.len()
    }
    /// Exact response occurrence currently owning one request family.
    pub(crate) fn response_claim_hash(
        &self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
    ) -> Option<HashOf<wire::CertifiedBodyResponse>> {
        self.response_claims.get(&request_hash).copied()
    }
    /// Validate one registration without changing either bounded index.
    pub(crate) fn plan_registration(
        &self,
        authenticated: AuthenticatedCertifiedBodyRequest,
    ) -> Result<CertifiedBodyRequestRegistrationPlan, V2TransportError> {
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
        Ok(CertifiedBodyRequestRegistrationPlan {
            incoming,
            identity,
            authenticated,
        })
    }
    /// Install a previously validated exact registration.
    ///
    /// The serialized tracker owner must not mutate either index between
    /// [`Self::plan_registration`] and this commit.
    pub(crate) fn commit_registration(&mut self, plan: CertifiedBodyRequestRegistrationPlan) {
        debug_assert!(!self.requests.contains_key(&plan.incoming));
        debug_assert!(!self.identities.contains_key(&plan.identity));
        debug_assert!(self.requests.len() < self.capacity);
        self.requests.insert(plan.incoming, plan.authenticated);
        self.identities.insert(plan.identity, plan.incoming);
    }
    /// Validate exact removal from both bounded indexes without changing them.
    pub(crate) fn plan_retirement(
        &self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
    ) -> Result<CertifiedBodyRequestRetirementPlan, V2TransportError> {
        let authenticated = self
            .requests
            .get(&request_hash)
            .cloned()
            .ok_or(V2TransportError::UnsolicitedResponse(request_hash))?;
        let identity = RequestIdentity::from(authenticated.request());
        if self.identities.get(&identity) != Some(&request_hash) {
            return Err(V2TransportError::InconsistentRequestIndex(request_hash));
        }
        Ok(CertifiedBodyRequestRetirementPlan {
            request_hash,
            identity,
        })
    }
    /// Remove one previously preflighted request from both indexes.
    ///
    /// This is structurally infallible because the serialized executor does
    /// not mutate the tracker between planning and commit. Keeping the exact
    /// request identity in the plan makes both removals independent of any
    /// later fallible lookup.
    pub(crate) fn commit_retirement(&mut self, plan: CertifiedBodyRequestRetirementPlan) {
        self.requests.remove(&plan.request_hash);
        self.identities.remove(&plan.identity);
        self.response_claims.remove(&plan.request_hash);
    }
    /// Register an authenticated request without eviction.
    ///
    /// Exact repeats and logically conflicting reissues are distinguished from
    /// capacity exhaustion and never mutate the tracker.
    ///
    /// # Errors
    ///
    /// Returns a duplicate, conflict, or capacity error as applicable.
    #[cfg(test)]
    pub(crate) fn register(
        &mut self,
        authenticated: AuthenticatedCertifiedBodyRequest,
    ) -> Result<(), V2TransportError> {
        let plan = self.plan_registration(authenticated)?;
        self.commit_registration(plan);
        Ok(())
    }
    /// Cancel one exact outstanding request and release its logical identity.
    ///
    /// View changes and locally recovered bodies can make a fetch unnecessary.
    /// Removing both indexes prevents abandoned requests from permanently
    /// consuming the bounded request capacity while late responses remain
    /// correctly classified as unsolicited.
    #[cfg(test)]
    pub(crate) fn cancel(&mut self, request_hash: HashOf<wire::CertifiedBodyRequest>) -> bool {
        let Ok(plan) = self.plan_retirement(request_hash) else {
            return false;
        };
        self.commit_retirement(plan);
        true
    }
    /// Complete one exact request after its authenticated body entered the
    /// reducer queue.
    #[cfg(test)]
    pub(crate) fn complete(&mut self, request_hash: HashOf<wire::CertifiedBodyRequest>) -> bool {
        let Ok(plan) = self.plan_retirement(request_hash) else {
            return false;
        };
        self.commit_retirement(plan);
        true
    }
    /// Authenticate a response for an outstanding exact request without
    /// consuming the request.
    ///
    /// # Errors
    ///
    /// Returns an error for unsolicited/replayed responses, malformed bodies
    /// or manifests, out-of-roster/spoofed responders, and invalid signatures.
    /// A frozen-roster historical archive peer does not have to be a signer of
    /// the old request QC: the verified QC authenticates the exact subject,
    /// while the response signature authenticates the peer serving the
    /// hash-bound canonical body.
    /// The outstanding request is retained on both success and error until the
    /// serialized executor explicitly completes or cancels it.
    pub(crate) fn authenticate_response(
        &self,
        context: &wire::HeightContext,
        response: wire::CertifiedBodyResponse,
        authenticated_responder: &PeerId,
    ) -> Result<AuthenticatedCertifiedBodyResponse, V2TransportError> {
        let request_hash = response.request_hash;
        let authenticated_request = self
            .requests
            .get(&request_hash)
            .ok_or(V2TransportError::UnsolicitedResponse(request_hash))?;
        authenticated_request.authenticate_response(context, response, authenticated_responder)
    }
    /// Check the one-response family without acquiring or replacing it.
    ///
    /// A vacant family and an exact retransmission remain eligible for the
    /// executor's later transactional admission preflight. A different
    /// authenticated response is rejected with
    /// [`V2TransportError::ConflictingCertifiedBodyResponseClaim`].
    pub(crate) fn preflight_authenticated_response_claim(
        &self,
        authenticated: &AuthenticatedCertifiedBodyResponse,
    ) -> Result<CertifiedBodyResponseClaimPreflight, V2TransportError> {
        let response = authenticated.response();
        let request_hash = response.request_hash;
        if !self.requests.contains_key(&request_hash) {
            return Err(V2TransportError::UnsolicitedResponse(request_hash));
        }
        let incoming = HashOf::new(response);
        match self.response_claims.get(&request_hash).copied() {
            Some(claimed) if claimed == incoming => {
                Ok(CertifiedBodyResponseClaimPreflight::ExactRetransmission)
            }
            Some(claimed) => Err(V2TransportError::ConflictingCertifiedBodyResponseClaim {
                request: request_hash,
                claimed,
                incoming,
            }),
            None => Ok(CertifiedBodyResponseClaimPreflight::Vacant),
        }
    }
    /// Freeze one authenticated response-family claim without changing it.
    ///
    /// The returned token owns the tracker's exclusive borrow. No safe caller
    /// can invalidate its request or claim preflight before consuming it, and
    /// dropping it is an exact stutter.
    pub(crate) fn prepare_authenticated_response_claim<'a>(
        &'a mut self,
        authenticated: &AuthenticatedCertifiedBodyResponse,
    ) -> Result<PreparedCertifiedBodyResponseClaim<'a>, V2TransportError> {
        let response = authenticated.response();
        let request_hash = response.request_hash;
        let response_hash = HashOf::new(response);
        if !self.validate_exact_indexes() {
            return Err(V2TransportError::InconsistentRequestIndex(request_hash));
        }
        let preflight = self.preflight_authenticated_response_claim(authenticated)?;
        Ok(PreparedCertifiedBodyResponseClaim {
            tracker: self,
            request_hash,
            response_hash,
            preflight,
        })
    }
    /// Claim the one physical response occurrence for a fully authenticated
    /// outstanding request.
    ///
    /// The caller must first perform [`Self::authenticate_response`] and its
    /// local pending-fetch lookup. Exact retransmission coalesces. A different
    /// responder or body cannot replace the acquired occurrence while the
    /// request remains outstanding. Reconstructing this tracker after restart
    /// deliberately restores requests but no volatile claims.
    pub(crate) fn claim_authenticated_response(
        &mut self,
        authenticated: &AuthenticatedCertifiedBodyResponse,
    ) -> Result<CertifiedBodyResponseClaimDisposition, V2TransportError> {
        Ok(self
            .prepare_authenticated_response_claim(authenticated)?
            .commit())
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
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.requests.len()
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
    use std::num::NonZeroU64;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
    use iroha_data_model::NetworkId;
    use iroha_data_model::block::{BlockHeader, BlockSignature, SignedBlock};
    use tempfile::TempDir;
    use crate::sumeragi::{v2_body_store::V2BodyStore, v2_chunks::encode_payload};
    use super::*;
    fn test_network_id(seed: u8) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([seed; Hash::LENGTH]),
        ))
    }
    struct Fixture {
        context: wire::HeightContext,
        validators: Vec<KeyPair>,
        observer: KeyPair,
        body: Vec<u8>,
        manifest: wire::PayloadManifest,
        chunks: Vec<Vec<u8>>,
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
                network_id: test_network_id(0x91),
                protocol_version: wire::PROTOCOL_VERSION,
                height: 1,
                epoch: 7,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: wire::ConsensusMode::Permissioned,
                parent_commit_qc: None,
                snapshot_bootstrap: None,
                quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
                roster,
                nexus_amx_context_hash: Hash::new(b"transport-test-nexus-amx-context"),
                execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
                da_layout: wire::DataAvailabilityLayout {
                    encoding: wire::PayloadEncoding::ReedSolomon16,
                    chunk_size_bytes: 64,
                    data_shards: 1,
                    parity_shards: 1,
                    max_payload_size_bytes: 4096,
                    max_chunk_count: 128,
                },
                leader_seed: [0x47; 32],
            };
            let round = wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 2,
            };
            let header = BlockHeader::new(
                NonZeroU64::new(context.height).expect("non-zero fixture height"),
                None,
                None,
                None,
                1_000,
                round.view,
            );
            let leader = context.leader(round.view);
            let leader_index = usize::try_from(leader).expect("small fixture leader index");
            let signature =
                SignatureOf::try_from_hash(validators[leader_index].private_key(), header.hash())
                    .expect("sign transport fixture proposal");
            let block = SignedBlock::presigned(
                BlockSignature::new(u64::from(leader), signature),
                header,
                Vec::new(),
            );
            let body = block
                .encode_wire()
                .expect("encode transport fixture proposal");
            let subject = wire::BlockSubject {
                parent_block_hash: None,
                block_hash: block.hash(),
                payload_hash: Hash::new(&body),
            };
            let (manifest, chunks) = encode_payload(&context, round, subject, &body)
                .expect("encode canonical fixture payload")
                .into_parts();
            let observer = KeyPair::try_from_seed(vec![90; 32], Algorithm::Ed25519)
                .expect("deterministic observer key");
            Self {
                context,
                validators,
                observer,
                body,
                manifest,
                chunks,
            }
        }
        fn peer(key: &KeyPair) -> PeerId {
            PeerId::new(key.public_key().clone())
        }
        fn signed_chunk(&self, sender: wire::ValidatorIndex) -> wire::PayloadChunk {
            let mut chunk = wire::PayloadChunk {
                manifest_hash: HashOf::new(&self.manifest),
                index: 0,
                bytes: self.chunks[0].clone(),
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
                    proposal_round: self.manifest.round,
                    phase: wire::GlobalPhase::Prepare,
                    subject: self.manifest.subject,
                    execution_commitment:
                        wire::ExecutionCommitment::without_topups_or_merge_carrier(
                            Hash::new(b"transport fixture parent state"),
                            Hash::new(b"transport fixture post state"),
                            Hash::new(b"transport fixture ordinary writes"),
                            1,
                            Hash::new(b"transport fixture executed block wire"),
                        ),
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
    fn reproposal_commit_qc_authenticates_its_exact_same_round_body() {
        let fixture = Fixture::new();
        let mut request = fixture.signed_request();
        request.certificate.phase = wire::GlobalPhase::Commit;
        request.round.view = request
            .round
            .view
            .checked_add(2)
            .expect("fixture reproposal view increment");
        request.certificate.round = request.round;
        request.certificate.proposal_round = request.round;
        request.signature = Signature::new(
            fixture.observer.private_key(),
            &request.signature_preimage(),
        )
        .payload()
        .to_vec();
        let authenticated = fixture
            .authenticate_request(request.clone())
            .expect("reproposal CommitQC authorizes its exact same-round body");
        let mut tracker = OutstandingCertifiedBodyRequests::new(1).expect("one request slot");
        tracker
            .register(authenticated)
            .expect("register exact reproposal request");
        let reproposal_manifest = wire::PayloadManifest::derive(
            &fixture.context,
            request.round,
            fixture.manifest.subject,
            u64::try_from(fixture.body.len()).expect("fixture body length"),
            &fixture.chunks,
        )
        .expect("derive exact reproposal manifest");
        let mut response = fixture.signed_response(&request, 0);
        response.manifest = reproposal_manifest;
        response.signature = Signature::new(
            fixture.validators[0].private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        let _authenticated = tracker
            .authenticate_response(
                &fixture.context,
                response,
                &Fixture::peer(&fixture.validators[0]),
            )
            .expect("authenticate exact reproposal response");
        let mut body_after_finality = request;
        body_after_finality.round.view = body_after_finality
            .certificate
            .round
            .view
            .checked_add(1)
            .expect("fixture body view increment");
        body_after_finality.signature = Signature::new(
            fixture.observer.private_key(),
            &body_after_finality.signature_preimage(),
        )
        .payload()
        .to_vec();
        assert!(matches!(
            fixture.authenticate_request(body_after_finality),
            Err(V2TransportError::Wire(
                wire::ValidationError::CertifiedBodyCertificateMismatch
            ))
        ));
    }
    #[test]
    fn response_authentication_never_consumes_before_explicit_completion() {
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
        let archive_response = fixture.signed_response(&request, 3);
        let archive_sender = Fixture::peer(&fixture.validators[3]);
        let authenticated_archive = tracker
            .authenticate_response(&fixture.context, archive_response.clone(), &archive_sender)
            .expect("frozen-roster archive need not have signed the old QC");
        assert_eq!(authenticated_archive.response(), &archive_response);
        let mut outside_roster = archive_response;
        outside_roster.responder =
            u32::try_from(fixture.context.roster.len()).expect("small fixture roster");
        outside_roster.signature = Signature::new(
            fixture.validators[3].private_key(),
            &outside_roster.signature_preimage(),
        )
        .payload()
        .to_vec();
        assert!(matches!(
            tracker.authenticate_response(&fixture.context, outside_roster, &archive_sender,),
            Err(V2TransportError::Wire(
                wire::ValidationError::SignerOutOfRange
            ))
        ));
        assert!(tracker.contains(request_hash));
        assert!(
            tracker.response_claims.is_empty(),
            "authentication alone cannot pin the occurrence slot"
        );
        let admitted = tracker
            .authenticate_response(&fixture.context, valid.clone(), &valid_sender)
            .expect("valid certified response");
        assert_eq!(admitted.response(), &valid);
        assert_eq!(
            tracker
                .claim_authenticated_response(&admitted)
                .expect("first authenticated response acquires its request slot"),
            CertifiedBodyResponseClaimDisposition::Acquired
        );
        assert!(tracker.contains(request_hash));
        let duplicate = tracker
            .authenticate_response(&fixture.context, valid.clone(), &valid_sender)
            .expect("authentication remains retryable before executor completion");
        assert_eq!(
            tracker
                .claim_authenticated_response(&duplicate)
                .expect("exact response retry coalesces"),
            CertifiedBodyResponseClaimDisposition::Coalesced
        );
        let competing = fixture.signed_response(&request, 1);
        let competing = tracker
            .authenticate_response(
                &fixture.context,
                competing,
                &Fixture::peer(&fixture.validators[1]),
            )
            .expect("second certified responder authenticates before claim arbitration");
        assert!(matches!(
            tracker.claim_authenticated_response(&competing),
            Err(V2TransportError::ConflictingCertifiedBodyResponseClaim {
                request,
                ..
            }) if request == request_hash
        ));
        assert_eq!(tracker.response_claims.len(), 1);
        assert!(tracker.complete(request_hash));
        assert!(tracker.is_empty());
        assert!(tracker.response_claims.is_empty());
        assert!(!tracker.complete(request_hash));
        assert_eq!(
            tracker.authenticate_response(&fixture.context, valid, &valid_sender),
            Err(V2TransportError::UnsolicitedResponse(request_hash))
        );
    }
    #[test]
    fn authenticated_certified_fetch_body_is_durable_and_exactly_bound() {
        let fixture = Fixture::new();
        let request = fixture.signed_request();
        let request_hash = HashOf::new(&request);
        let response = fixture.signed_response(&request, 0);
        let response_hash = HashOf::new(&response);
        let manifest_hash = HashOf::new(&response.manifest);
        let mut tracker =
            OutstandingCertifiedBodyRequests::new(1).expect("one exact request family");
        tracker
            .register(
                fixture
                    .authenticate_request(request.clone())
                    .expect("authenticate durable-body request"),
            )
            .expect("register durable-body request");
        let authenticated = tracker
            .authenticate_response(
                &fixture.context,
                response,
                &Fixture::peer(&fixture.validators[0]),
            )
            .expect("authenticate durable-body response");
        let directory = TempDir::new().expect("temporary durable-body directory");
        let mut body_store = V2BodyStore::open(directory.path(), fixture.context.clone())
            .expect("open durable-body store");
        let receipt = body_store
            .persist_authenticated_certified_fetch_response(&authenticated)
            .expect("persist authenticated response body");
        assert_eq!(receipt.request_hash(), request_hash);
        assert_eq!(receipt.response_hash(), response_hash);
        assert_eq!(receipt.durable_body().context_id(), fixture.context.id());
        assert_eq!(receipt.durable_body().round(), fixture.manifest.round);
        assert_eq!(receipt.durable_body().subject(), fixture.manifest.subject);
        assert_eq!(receipt.durable_body().manifest_hash(), manifest_hash);
        let repeated = body_store
            .persist_authenticated_certified_fetch_response(&authenticated)
            .expect("exact response repeat is idempotent");
        assert_eq!(repeated, receipt);
        // A second legitimate responder changes the authenticated transport
        // occurrence, but cannot replace the hash-bound body-store frame. A
        // genuinely different body under this same subject cannot pass
        // response authentication without breaking the payload hash.
        let other_response = fixture.signed_response(&request, 1);
        let other_response_hash = HashOf::new(&other_response);
        let other_authenticated = tracker
            .authenticate_response(
                &fixture.context,
                other_response,
                &Fixture::peer(&fixture.validators[1]),
            )
            .expect("authenticate second durable-body responder");
        let other_receipt = body_store
            .persist_authenticated_certified_fetch_response(&other_authenticated)
            .expect("same exact body from another responder is idempotent");
        assert_eq!(other_receipt.request_hash(), request_hash);
        assert_eq!(other_receipt.response_hash(), other_response_hash);
        assert_ne!(other_receipt.response_hash(), receipt.response_hash());
        assert_eq!(other_receipt.durable_body(), receipt.durable_body());
        let durable_body = receipt.durable_body().clone();
        drop(body_store);
        let reopened = V2BodyStore::open(directory.path(), fixture.context.clone())
            .expect("reopen durable-body store");
        let recovered = reopened
            .receipt(fixture.manifest.round, fixture.manifest.subject)
            .expect("recover exact durable body receipt");
        assert_eq!(recovered, durable_body);
        assert_eq!(
            reopened
                .load_canonical_wire(&recovered)
                .expect("load recovered canonical body"),
            fixture.body
        );
    }
    #[test]
    fn prepared_response_claim_is_drop_safe_and_commits_without_repreflight() {
        let fixture = Fixture::new();
        let request = fixture.signed_request();
        let request_hash = HashOf::new(&request);
        let response = fixture.signed_response(&request, 0);
        let response_hash = HashOf::new(&response);
        let responder = Fixture::peer(&fixture.validators[0]);
        let mut tracker =
            OutstandingCertifiedBodyRequests::new(1).expect("one exact request family");
        tracker
            .register(
                fixture
                    .authenticate_request(request)
                    .expect("authenticate prepared-claim request"),
            )
            .expect("register prepared-claim request");
        let authenticated = tracker
            .authenticate_response(&fixture.context, response, &responder)
            .expect("authenticate prepared-claim response");
        let claims_before = tracker.response_claims.clone();
        let prepared = tracker
            .prepare_authenticated_response_claim(&authenticated)
            .expect("vacant response family prepares");
        assert_eq!(
            prepared.preflight(),
            CertifiedBodyResponseClaimPreflight::Vacant
        );
        assert_eq!(prepared.request_hash(), request_hash);
        assert_eq!(prepared.response_hash(), response_hash);
        drop(prepared);
        assert_eq!(tracker.response_claims, claims_before);
        let disposition = tracker
            .prepare_authenticated_response_claim(&authenticated)
            .expect("unchanged vacant family prepares again")
            .commit();
        assert_eq!(disposition, CertifiedBodyResponseClaimDisposition::Acquired);
        assert_eq!(
            tracker.response_claim_hash(request_hash),
            Some(response_hash)
        );
        let retransmission = tracker
            .prepare_authenticated_response_claim(&authenticated)
            .expect("exact claimed response prepares as a retransmission");
        assert_eq!(
            retransmission.preflight(),
            CertifiedBodyResponseClaimPreflight::ExactRetransmission
        );
        assert_eq!(
            retransmission.commit(),
            CertifiedBodyResponseClaimDisposition::Coalesced
        );
        assert_eq!(tracker.response_claim_count(), 1);
        assert_eq!(
            tracker.response_claim_hash(request_hash),
            Some(response_hash)
        );
        let identity = RequestIdentity::from(tracker.requests[&request_hash].request());
        let removed = tracker
            .identities
            .remove(&identity)
            .expect("remove exact reverse request index for prepared-claim negative");
        let claims_before = tracker.response_claims.clone();
        assert!(matches!(
            tracker.prepare_authenticated_response_claim(&authenticated),
            Err(V2TransportError::InconsistentRequestIndex(hash)) if hash == request_hash
        ));
        assert_eq!(tracker.response_claims, claims_before);
        tracker.identities.insert(identity, removed);
        assert!(tracker.validate_exact_indexes());
    }
    #[test]
    fn response_claim_is_bounded_by_request_and_reopens_only_after_restart() {
        let fixture = Fixture::new();
        let request = fixture.signed_request();
        let request_hash = HashOf::new(&request);
        let authenticated_request = fixture
            .authenticate_request(request.clone())
            .expect("authenticate restart request");
        let first_response = fixture.signed_response(&request, 0);
        let first_sender = Fixture::peer(&fixture.validators[0]);
        let mut first =
            OutstandingCertifiedBodyRequests::new(1).expect("one shared request/claim slot");
        first
            .register(authenticated_request.clone())
            .expect("register restart request");
        let first_authenticated = first
            .authenticate_response(&fixture.context, first_response, &first_sender)
            .expect("authenticate first response");
        assert_eq!(
            first
                .claim_authenticated_response(&first_authenticated)
                .expect("acquire first response occurrence"),
            CertifiedBodyResponseClaimDisposition::Acquired
        );
        assert_eq!(first.len(), 1);
        assert_eq!(first.response_claims.len(), 1);
        let competing_response = fixture.signed_response(&request, 1);
        let competing_sender = Fixture::peer(&fixture.validators[1]);
        let competing_authenticated = first
            .authenticate_response(
                &fixture.context,
                competing_response.clone(),
                &competing_sender,
            )
            .expect("authenticate competing response");
        assert!(matches!(
            first.claim_authenticated_response(&competing_authenticated),
            Err(V2TransportError::ConflictingCertifiedBodyResponseClaim {
                request,
                ..
            }) if request == request_hash
        ));
        // Same-height recovery reconstructs the outstanding logical request,
        // but the unconsumed physical response occurrence is intentionally
        // volatile. A different certified responder may acquire it anew.
        let mut restarted =
            OutstandingCertifiedBodyRequests::new(1).expect("one restarted request/claim slot");
        restarted
            .register(authenticated_request)
            .expect("restore outstanding request without its volatile claim");
        assert!(restarted.response_claims.is_empty());
        let after_restart = restarted
            .authenticate_response(&fixture.context, competing_response, &competing_sender)
            .expect("authenticate response after restart");
        assert_eq!(
            restarted
                .claim_authenticated_response(&after_restart)
                .expect("response can reclaim the shared request slot after restart"),
            CertifiedBodyResponseClaimDisposition::Acquired
        );
        assert!(restarted.complete(request_hash));
        assert!(restarted.response_claims.is_empty());
        assert!(matches!(
            restarted.claim_authenticated_response(&after_restart),
            Err(V2TransportError::UnsolicitedResponse(hash)) if hash == request_hash
        ));
    }
    #[test]
    fn certified_request_index_validation_rejects_missing_reverse_and_foreign_claim() {
        let fixture = Fixture::new();
        let request = fixture.signed_request();
        let authenticated = fixture
            .authenticate_request(request.clone())
            .expect("authenticate exact request");
        let mut tracker =
            OutstandingCertifiedBodyRequests::new(2).expect("non-zero tracker capacity");
        tracker
            .register(authenticated)
            .expect("register exact request");
        assert!(tracker.validate_exact_indexes());
        let identity = RequestIdentity::from(&request);
        let request_hash = tracker
            .identities
            .remove(&identity)
            .expect("remove exact reverse index for negative test");
        assert!(!tracker.validate_exact_indexes());
        tracker.identities.insert(identity, request_hash);
        assert!(tracker.validate_exact_indexes());
        let foreign_request =
            HashOf::from_untyped_unchecked(Hash::new(b"foreign request response-claim index"));
        let foreign_response =
            HashOf::from_untyped_unchecked(Hash::new(b"foreign response response-claim index"));
        tracker
            .response_claims
            .insert(foreign_request, foreign_response);
        assert!(!tracker.validate_exact_indexes());
        tracker.response_claims.remove(&foreign_request);
        assert!(tracker.validate_exact_indexes());
    }
    #[test]
    fn unsolicited_response_is_rejected_without_state() {
        let fixture = Fixture::new();
        let request = fixture.signed_request();
        let response = fixture.signed_response(&request, 0);
        let sender = Fixture::peer(&fixture.validators[0]);
        let tracker = OutstandingCertifiedBodyRequests::new(1).expect("positive capacity");
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
        second.certificate.proposal_round = second.round;
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
    #[test]
    fn registration_plan_is_exact_atomic_and_requires_one_commit() {
        let fixture = Fixture::new();
        let request = fixture.signed_request();
        let request_hash = HashOf::new(&request);
        let authenticated = fixture
            .authenticate_request(request.clone())
            .expect("authenticate request");
        let mut tracker = OutstandingCertifiedBodyRequests::new(2).expect("positive capacity");
        let empty_hashes = tracker.hashes();
        let empty_identities = tracker.identities.clone();
        let plan = tracker
            .plan_registration(authenticated.clone())
            .expect("plan exact registration");
        assert_eq!(tracker.hashes(), empty_hashes);
        assert_eq!(tracker.identities, empty_identities);
        tracker.commit_registration(plan);
        assert_eq!(tracker.hashes(), BTreeSet::from([request_hash]));
        assert_eq!(tracker.identities.len(), 1);
        let committed_hashes = tracker.hashes();
        let committed_identities = tracker.identities.clone();
        assert_eq!(
            tracker.plan_registration(authenticated),
            Err(V2TransportError::DuplicateRequest(request_hash))
        );
        assert_eq!(tracker.hashes(), committed_hashes);
        assert_eq!(tracker.identities, committed_identities);
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
            tracker.plan_registration(
                fixture
                    .authenticate_request(conflicting)
                    .expect("authenticate conflicting request")
            ),
            Err(V2TransportError::ConflictingRequest {
                existing: request_hash,
                incoming: conflicting_hash,
            })
        );
        assert_eq!(tracker.hashes(), committed_hashes);
        assert_eq!(tracker.identities, committed_identities);
        let mut capacity_tracker =
            OutstandingCertifiedBodyRequests::new(1).expect("positive capacity");
        capacity_tracker
            .register(
                fixture
                    .authenticate_request(request.clone())
                    .expect("authenticate capacity owner"),
            )
            .expect("fill request capacity");
        let capacity_hashes = capacity_tracker.hashes();
        let capacity_identities = capacity_tracker.identities.clone();
        let mut second = request;
        second.round.view += 1;
        second.certificate.round = second.round;
        second.certificate.proposal_round = second.round;
        second.signature =
            Signature::new(fixture.observer.private_key(), &second.signature_preimage())
                .payload()
                .to_vec();
        assert_eq!(
            capacity_tracker.plan_registration(
                fixture
                    .authenticate_request(second)
                    .expect("authenticate over-capacity request")
            ),
            Err(V2TransportError::CapacityExceeded { capacity: 1 })
        );
        assert_eq!(capacity_tracker.hashes(), capacity_hashes);
        assert_eq!(capacity_tracker.identities, capacity_identities);
    }
}
