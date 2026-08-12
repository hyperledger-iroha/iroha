//! Crash-safe, scheduler-free payload storage for Certified-Serve lifecycles.
//!
//! One canonical file is owned by the hash of one exact signed
//! [`wire::CertifiedBodyRequest`]. The full request is retained so startup can
//! independently reauthenticate it. Completed records retain only response
//! metadata: canonical body bytes remain owned by the v2 body store and must be
//! resolved there before a response is reconstructed.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    mem::size_of,
    path::{Path, PathBuf},
};

use iroha_crypto::{Hash, HashOf, KeyPair, Signature};
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::{Decode, DecodeAll as _, Encode};
use thiserror::Error;

#[cfg(any(not(test), feature = "bls"))]
use super::v2_body_store::DurableBodyReceipt;
use super::{
    v2::VerifiedHeightContext,
    v2_body_store::V2BodyStore,
    v2_transport::{
        AuthenticatedCertifiedBodyRequest, authenticate_certified_body_request_identity,
        authenticate_certified_body_request_with_verified_height,
    },
};

const STORE_DIRECTORY: &str = "certified-serve-payload-v1";
const FILE_SUFFIX: &str = ".norito";
const TEMPORARY_FILE_SUFFIX: &str = ".norito.tmp";
const FRAME_MAGIC: &[u8; 8] = b"SUM2SRV1";
const FORMAT_VERSION: u16 = 1;
const CHECKSUM_BYTES: usize = Hash::LENGTH;
const FRAME_HEADER_BYTES: usize =
    FRAME_MAGIC.len() + size_of::<u16>() + size_of::<u64>() + CHECKSUM_BYTES;
const ENTRY_FIXED_HEADROOM_BYTES: u64 = 64 * 1024;

/// Hard per-height bound for exact Certified-Serve payloads.
///
/// Every Serve admission owns an adjacent reserved ProducerTurn in the
/// lifecycle ledger, so at most half of the closed `u16`-sized record space can
/// own payload files.
pub(crate) const MAX_CERTIFIED_SERVE_PAYLOADS_PER_HEIGHT: usize = (u16::MAX as usize + 1) / 2;

/// Exact durable identity of one signed Certified-Serve request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct CertifiedServePayloadId(HashOf<wire::CertifiedBodyRequest>);

impl CertifiedServePayloadId {
    /// Return the hash of the exact signed request.
    pub(crate) const fn request_hash(self) -> HashOf<wire::CertifiedBodyRequest> {
        self.0
    }

    fn from_request(request: &wire::CertifiedBodyRequest) -> Self {
        Self(HashOf::new(request))
    }
}

/// Typed terminal negative result for an exact Certified-Serve request.
///
/// The variant is part of the durable meaning: a rejection and a failed local
/// service attempt with the same numeric code are never interchangeable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) enum CertifiedServePayloadNegativeOutcome {
    /// The request was deterministically cancelled.
    #[codec(index = 0)]
    Cancelled,
    /// Authentication or policy rejected the request with a closed code.
    #[codec(index = 1)]
    Rejected(u16),
    /// Local service failed terminally with a closed code.
    #[codec(index = 2)]
    Failed(u16),
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
enum PersistedCertifiedServePayloadStateV1 {
    #[codec(index = 0)]
    Pending,
    #[codec(index = 1)]
    Completed {
        response_hash: HashOf<wire::CertifiedBodyResponse>,
        manifest: wire::PayloadManifest,
        responder: wire::ValidatorIndex,
        signature: Vec<u8>,
    },
    #[codec(index = 2)]
    Negative {
        outcome: CertifiedServePayloadNegativeOutcome,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
#[norito(deny_unknown_fields)]
struct PersistedCertifiedServePayloadV1 {
    format_version: u16,
    context_id: wire::HeightContextId,
    height: wire::Height,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    request: wire::CertifiedBodyRequest,
    state: PersistedCertifiedServePayloadStateV1,
}

impl PersistedCertifiedServePayloadV1 {
    fn id(&self) -> CertifiedServePayloadId {
        CertifiedServePayloadId(self.request_hash)
    }

    fn payload_hash(&self) -> Hash {
        Hash::new(self.encode())
    }
}

/// Receipt proving that one exact request exists in the durable payload store.
///
/// Fresh receipts name Pending frames. Tombstone replay may mint the same
/// request-level proof from an already durable terminal frame without changing
/// that frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct DurableCertifiedServeAdmissionReceipt {
    id: CertifiedServePayloadId,
    certificate_hash: HashOf<wire::QuorumCertificate>,
    payload_hash: Hash,
    local_retainer: wire::ValidatorIndex,
}

impl DurableCertifiedServeAdmissionReceipt {
    /// Exact signed-request identity covered by the receipt.
    pub(crate) const fn id(self) -> CertifiedServePayloadId {
        self.id
    }

    /// Hash of the exact certificate carried by the signed request.
    pub(crate) const fn certificate_hash(self) -> HashOf<wire::QuorumCertificate> {
        self.certificate_hash
    }

    /// Hash of the canonical persisted payload protected by the frame.
    pub(crate) const fn payload_hash(self) -> Hash {
        self.payload_hash
    }

    /// Frozen-roster validator whose certified retention authority was checked
    /// before this receipt reached durable storage.
    pub(crate) const fn local_retainer(self) -> wire::ValidatorIndex {
        self.local_retainer
    }
}

/// Sealed result of retaining one exact request for lifecycle admission.
///
/// A pending publication may be compensated after a conclusive rejection. A
/// terminal publication is valid only when the coordinator already owns the
/// matching durable row and can replay its tombstone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DurableCertifiedServeAdmissionPublication {
    receipt: DurableCertifiedServeAdmissionReceipt,
    pending: bool,
}

impl DurableCertifiedServeAdmissionPublication {
    /// Exact durable request material used to project the lifecycle candidate.
    pub(super) const fn receipt(self) -> DurableCertifiedServeAdmissionReceipt {
        self.receipt
    }

    /// Whether this publication may be removed as an unadmitted Pending frame.
    pub(super) const fn is_pending(self) -> bool {
        self.pending
    }
}

/// Receipt minted only after completed response metadata is durable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct DurableCertifiedServeCompletedReceipt {
    id: CertifiedServePayloadId,
    certificate_hash: HashOf<wire::QuorumCertificate>,
    payload_hash: Hash,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
}

impl DurableCertifiedServeCompletedReceipt {
    /// Exact signed-request identity covered by the receipt.
    pub(crate) const fn id(self) -> CertifiedServePayloadId {
        self.id
    }

    /// Hash of the exact certificate carried by the signed request.
    pub(crate) const fn certificate_hash(self) -> HashOf<wire::QuorumCertificate> {
        self.certificate_hash
    }

    /// Hash of the canonical persisted payload protected by the frame.
    pub(crate) const fn payload_hash(self) -> Hash {
        self.payload_hash
    }

    /// Hash of the complete authenticated response, including its body bytes.
    pub(crate) const fn response_hash(self) -> HashOf<wire::CertifiedBodyResponse> {
        self.response_hash
    }
}

/// Receipt minted only after a deterministic negative result is durable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct DurableCertifiedServeNegativeReceipt {
    id: CertifiedServePayloadId,
    certificate_hash: HashOf<wire::QuorumCertificate>,
    payload_hash: Hash,
    outcome: CertifiedServePayloadNegativeOutcome,
}

impl DurableCertifiedServeNegativeReceipt {
    /// Exact signed-request identity covered by the receipt.
    pub(crate) const fn id(self) -> CertifiedServePayloadId {
        self.id
    }

    /// Hash of the exact certificate carried by the signed request.
    pub(crate) const fn certificate_hash(self) -> HashOf<wire::QuorumCertificate> {
        self.certificate_hash
    }

    /// Hash of the canonical persisted payload protected by the frame.
    pub(crate) const fn payload_hash(self) -> Hash {
        self.payload_hash
    }

    /// Deterministic terminal result covered by the receipt.
    pub(crate) const fn outcome(self) -> CertifiedServePayloadNegativeOutcome {
        self.outcome
    }
}

/// Body-independent reference recovered for one completed response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RecoveredCertifiedServeCompletedPayload<'a> {
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    manifest: &'a wire::PayloadManifest,
    responder: wire::ValidatorIndex,
    signature: &'a [u8],
}

impl RecoveredCertifiedServeCompletedPayload<'_> {
    /// Hash of the original complete response.
    pub(crate) const fn response_hash(&self) -> HashOf<wire::CertifiedBodyResponse> {
        self.response_hash
    }

    /// Manifest used to resolve canonical body bytes from the body store.
    pub(crate) const fn manifest(&self) -> &wire::PayloadManifest {
        self.manifest
    }

    /// Frozen-roster responder index that signed the response.
    pub(crate) const fn responder(&self) -> wire::ValidatorIndex {
        self.responder
    }

    /// Original responder signature.
    pub(crate) const fn signature(&self) -> &[u8] {
        self.signature
    }
}

/// Closed recovered state of one Certified-Serve payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    variant_size_differences,
    clippy::large_enum_variant,
    reason = "the recovered state is a Copy borrowed view; boxing would allocate during startup recovery"
)]
pub(crate) enum RecoveredCertifiedServePayloadState<'a> {
    /// The exact request was admitted but has no durable terminal result.
    Pending,
    /// Response metadata is durable; body bytes must be resolved separately.
    Completed(RecoveredCertifiedServeCompletedPayload<'a>),
    /// The request reached a deterministic negative terminal result.
    Negative(CertifiedServePayloadNegativeOutcome),
}

/// Borrowed view of one entry in a startup recovery cut.
#[derive(Clone, Copy, Debug)]
#[must_use]
pub(crate) struct RecoveredCertifiedServePayload<'a> {
    payload: &'a PersistedCertifiedServePayloadV1,
}

impl RecoveredCertifiedServePayload<'_> {
    /// Exact signed-request identity.
    pub(crate) fn id(&self) -> CertifiedServePayloadId {
        self.payload.id()
    }

    /// Full signed request retained for independent startup authentication.
    pub(crate) const fn request(&self) -> &wire::CertifiedBodyRequest {
        &self.payload.request
    }

    /// Hash of the exact certificate carried by the recovered request.
    pub(crate) fn certificate_hash(&self) -> HashOf<wire::QuorumCertificate> {
        HashOf::new(&self.payload.request.certificate)
    }

    /// Durable state recovered for the request.
    pub(crate) fn state(&self) -> RecoveredCertifiedServePayloadState<'_> {
        match &self.payload.state {
            PersistedCertifiedServePayloadStateV1::Pending => {
                RecoveredCertifiedServePayloadState::Pending
            }
            PersistedCertifiedServePayloadStateV1::Completed {
                response_hash,
                manifest,
                responder,
                signature,
            } => RecoveredCertifiedServePayloadState::Completed(
                RecoveredCertifiedServeCompletedPayload {
                    response_hash: *response_hash,
                    manifest,
                    responder: *responder,
                    signature,
                },
            ),
            PersistedCertifiedServePayloadStateV1::Negative { outcome } => {
                RecoveredCertifiedServePayloadState::Negative(*outcome)
            }
        }
    }
}

/// Fully reauthenticated response reconstructed from the payload and body
/// stores during startup.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct AuthenticatedRecoveredCertifiedServeResponse {
    response: wire::CertifiedBodyResponse,
}

impl AuthenticatedRecoveredCertifiedServeResponse {
    /// Borrow the exact reconstructed and signature-verified response.
    pub(crate) const fn response(&self) -> &wire::CertifiedBodyResponse {
        &self.response
    }
}

/// Closed post-authentication state of one recovered Serve payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AuthenticatedRecoveredCertifiedServePayloadState {
    /// The exact request remains pending physical execution.
    Pending,
    /// A complete response was reconstructed from independently durable body
    /// bytes and reauthenticated.
    Completed(AuthenticatedRecoveredCertifiedServeResponse),
    /// A typed local terminal outcome was durably recorded after admission.
    Negative(CertifiedServePayloadNegativeOutcome),
}

/// One recovered payload whose request QC, local retention authority, and any
/// completed response have been independently reauthenticated.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct AuthenticatedRecoveredCertifiedServePayload {
    request: AuthenticatedCertifiedBodyRequest,
    payload_hash: Hash,
    state: AuthenticatedRecoveredCertifiedServePayloadState,
}

impl AuthenticatedRecoveredCertifiedServePayload {
    /// Exact signed-request identity.
    pub(crate) const fn id(&self) -> CertifiedServePayloadId {
        CertifiedServePayloadId(self.request.request_hash())
    }

    /// Borrow the fully authenticated request.
    pub(crate) const fn request(&self) -> &AuthenticatedCertifiedBodyRequest {
        &self.request
    }

    /// Hash of the request's fully authenticated quorum certificate.
    pub(crate) fn certificate_hash(&self) -> HashOf<wire::QuorumCertificate> {
        HashOf::new(&self.request.request().certificate)
    }

    /// Hash of the canonical payload-store record protected by its frame.
    pub(crate) const fn payload_hash(&self) -> Hash {
        self.payload_hash
    }

    /// Borrow the exact post-authentication recovery state.
    pub(crate) const fn state(&self) -> &AuthenticatedRecoveredCertifiedServePayloadState {
        &self.state
    }
}

/// Move-only, post-authentication snapshot of every bounded Serve payload for
/// one verified height.
#[derive(Debug)]
#[must_use]
pub(crate) struct AuthenticatedCertifiedServePayloadRecoveryCut {
    context_id: wire::HeightContextId,
    height: wire::Height,
    payloads: BTreeMap<CertifiedServePayloadId, AuthenticatedRecoveredCertifiedServePayload>,
}

impl AuthenticatedCertifiedServePayloadRecoveryCut {
    /// Frozen verified height-context identity owning this cut.
    pub(crate) const fn context_id(&self) -> wire::HeightContextId {
        self.context_id
    }

    /// Exact verified height owning this cut.
    pub(crate) const fn height(&self) -> wire::Height {
        self.height
    }

    /// Number of independently authenticated payloads.
    pub(crate) fn len(&self) -> usize {
        self.payloads.len()
    }

    /// Whether no payload survived authenticated recovery.
    pub(crate) fn is_empty(&self) -> bool {
        self.payloads.is_empty()
    }

    /// Resolve one authenticated payload by exact signed-request identity.
    pub(crate) fn get(
        &self,
        id: CertifiedServePayloadId,
    ) -> Option<&AuthenticatedRecoveredCertifiedServePayload> {
        self.payloads.get(&id)
    }

    /// Iterate in canonical signed-request-hash order.
    pub(crate) fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = &AuthenticatedRecoveredCertifiedServePayload> {
        self.payloads.values()
    }
}

/// Move-only startup snapshot of every bounded Certified-Serve payload file.
///
/// Records in this cut are structurally validated and requester-signed, but
/// their quorum certificates and completed body references still require
/// independent authentication before coordinator reconstruction.
#[derive(Debug)]
#[must_use]
pub(crate) struct CertifiedServePayloadRecoveryCut {
    context_id: wire::HeightContextId,
    height: wire::Height,
    payloads: BTreeMap<CertifiedServePayloadId, PersistedCertifiedServePayloadV1>,
}

impl CertifiedServePayloadRecoveryCut {
    /// Frozen height-context identity owning this cut.
    pub(crate) const fn context_id(&self) -> wire::HeightContextId {
        self.context_id
    }

    /// Exact consensus height owning this cut.
    pub(crate) const fn height(&self) -> wire::Height {
        self.height
    }

    /// Number of recovered exact requests.
    pub(crate) fn len(&self) -> usize {
        self.payloads.len()
    }

    /// Whether no Certified-Serve payload was recovered.
    pub(crate) fn is_empty(&self) -> bool {
        self.payloads.is_empty()
    }

    /// Resolve one recovered request by its exact signed-request hash.
    pub(crate) fn get(
        &self,
        id: CertifiedServePayloadId,
    ) -> Option<RecoveredCertifiedServePayload<'_>> {
        self.payloads
            .get(&id)
            .map(|payload| RecoveredCertifiedServePayload { payload })
    }

    /// Iterate in canonical request-hash order.
    pub(crate) fn iter(
        &self,
    ) -> impl ExactSizeIterator<Item = RecoveredCertifiedServePayload<'_>> + '_ {
        self.payloads
            .values()
            .map(|payload| RecoveredCertifiedServePayload { payload })
    }

    /// Reauthenticate every recovered request and completed response before
    /// the lifecycle ledger is allowed to join it.
    ///
    /// # Errors
    ///
    /// Returns an error for a foreign verified context, invalid request QC,
    /// lost local retention authority, missing/corrupt canonical body, or a
    /// response whose identity or signature no longer matches its payload.
    pub(crate) fn authenticate(
        self,
        verified: &VerifiedHeightContext,
        local_signer: &KeyPair,
        body_store: &V2BodyStore,
    ) -> Result<AuthenticatedCertifiedServePayloadRecoveryCut, CertifiedServePayloadRecoveryError>
    {
        let context = verified.context();
        if self.context_id != context.id() || self.height != context.height {
            return Err(CertifiedServePayloadRecoveryError::ForeignContext);
        }
        if !body_store.matches_context(context) {
            return Err(CertifiedServePayloadRecoveryError::ForeignBodyStore);
        }
        if self.payloads.is_empty() {
            return Ok(AuthenticatedCertifiedServePayloadRecoveryCut {
                context_id: self.context_id,
                height: self.height,
                payloads: BTreeMap::new(),
            });
        }
        let local_retainer = context
            .roster
            .iter()
            .position(|entry| entry.validator.public_key() == local_signer.public_key())
            .and_then(|index| wire::ValidatorIndex::try_from(index).ok())
            .ok_or(CertifiedServePayloadRecoveryError::LocalRetentionAuthorityAbsent)?;
        let mut authenticated = BTreeMap::new();
        for (id, payload) in self.payloads {
            let payload_hash = payload.payload_hash();
            let requester = payload.request.requester.clone();
            let request = authenticate_certified_body_request_with_verified_height(
                verified,
                payload.request.clone(),
                &requester,
            )
            .map_err(|error| {
                CertifiedServePayloadRecoveryError::InvalidRequest(error.to_string())
            })?;
            if request.request_hash() != id.request_hash()
                || request
                    .request()
                    .certificate
                    .signers
                    .binary_search(&local_retainer)
                    .is_err()
            {
                return Err(CertifiedServePayloadRecoveryError::LocalRetentionAuthorityAbsent);
            }
            let state = match payload.state {
                PersistedCertifiedServePayloadStateV1::Pending => {
                    AuthenticatedRecoveredCertifiedServePayloadState::Pending
                }
                PersistedCertifiedServePayloadStateV1::Negative { outcome } => {
                    AuthenticatedRecoveredCertifiedServePayloadState::Negative(outcome)
                }
                PersistedCertifiedServePayloadStateV1::Completed {
                    response_hash,
                    manifest,
                    responder: persisted_responder,
                    signature,
                } => {
                    let responder_index = usize::try_from(persisted_responder)
                        .ok()
                        .filter(|index| *index < context.roster.len())
                        .ok_or(CertifiedServePayloadRecoveryError::InvalidResponse(
                            "persisted responder is outside the frozen roster".to_owned(),
                        ))?;
                    let responder_peer = &context.roster[responder_index].validator;
                    let (stored_manifest, receipt) = body_store
                        .recovered(request.request().round, request.request().subject)
                        .map_err(|error| {
                            CertifiedServePayloadRecoveryError::InvalidBody(error.to_string())
                        })?
                        .ok_or(CertifiedServePayloadRecoveryError::MissingBody)?;
                    if stored_manifest != manifest {
                        return Err(CertifiedServePayloadRecoveryError::ManifestMismatch);
                    }
                    let body = body_store.load_canonical_wire(&receipt).map_err(|error| {
                        CertifiedServePayloadRecoveryError::InvalidBody(error.to_string())
                    })?;
                    let response = wire::CertifiedBodyResponse {
                        request_hash: request.request_hash(),
                        manifest,
                        body,
                        responder: persisted_responder,
                        signature,
                    };
                    response
                        .validate_against(context, request.request(), responder_peer)
                        .map_err(|error| {
                            CertifiedServePayloadRecoveryError::InvalidResponse(error.to_string())
                        })?;
                    let response_signature = Signature::try_from_bytes(&response.signature)
                        .map_err(|error| {
                            CertifiedServePayloadRecoveryError::InvalidResponse(error.to_string())
                        })?;
                    response_signature
                        .verify(responder_peer.public_key(), &response.signature_preimage())
                        .map_err(|error| {
                            CertifiedServePayloadRecoveryError::InvalidResponse(error.to_string())
                        })?;
                    if HashOf::new(&response) != response_hash {
                        return Err(CertifiedServePayloadRecoveryError::ResponseHashMismatch);
                    }
                    AuthenticatedRecoveredCertifiedServePayloadState::Completed(
                        AuthenticatedRecoveredCertifiedServeResponse { response },
                    )
                }
            };
            let recovered = AuthenticatedRecoveredCertifiedServePayload {
                request,
                payload_hash,
                state,
            };
            if authenticated.insert(id, recovered).is_some() {
                return Err(CertifiedServePayloadRecoveryError::DuplicateRequestHash);
            }
        }
        Ok(AuthenticatedCertifiedServePayloadRecoveryCut {
            context_id: self.context_id,
            height: self.height,
            payloads: authenticated,
        })
    }
}

/// Failure while independently authenticating a payload recovery cut.
#[derive(Debug, Error)]
pub(crate) enum CertifiedServePayloadRecoveryError {
    /// The cut belongs to another verified height context.
    #[error("Certified-Serve payload recovery cut belongs to another height context")]
    ForeignContext,
    /// The supplied body store belongs to another height context.
    #[error("Certified-Serve payload recovery used a foreign body store")]
    ForeignBodyStore,
    /// No local validator in the frozen roster owns the retained Serve work.
    #[error("Certified-Serve payload lost local certified retention authority")]
    LocalRetentionAuthorityAbsent,
    /// A persisted request failed requester or quorum-certificate authentication.
    #[error("invalid recovered Certified-Serve request: {0}")]
    InvalidRequest(String),
    /// A pending completed response lost its exact canonical body.
    #[error("recovered Certified-Serve response lost its canonical body")]
    MissingBody,
    /// Canonical body-store access failed.
    #[error("invalid recovered Certified-Serve body: {0}")]
    InvalidBody(String),
    /// Payload and body stores disagree on the exact manifest.
    #[error("recovered Certified-Serve manifest differs from its body store")]
    ManifestMismatch,
    /// A reconstructed response failed structural or signature authentication.
    #[error("invalid recovered Certified-Serve response: {0}")]
    InvalidResponse(String),
    /// Reconstructed full response bytes do not match the durable response hash.
    #[error("recovered Certified-Serve response hash mismatch")]
    ResponseHashMismatch,
    /// Canonical recovery unexpectedly produced a duplicate request hash.
    #[error("duplicate authenticated Certified-Serve request hash")]
    DuplicateRequestHash,
}

/// Failure while opening or advancing the Certified-Serve payload store.
#[derive(Debug, Error)]
pub(crate) enum CertifiedServePayloadStoreError {
    /// A filesystem operation failed.
    #[error("failed to {operation} Certified-Serve payload path {}: {source}", path.display())]
    Io {
        /// Short operation description.
        operation: &'static str,
        /// Path involved in the failed operation.
        path: PathBuf,
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },
    /// Immutable storage geometry could not be represented safely.
    #[error("invalid Certified-Serve payload geometry: {0}")]
    InvalidGeometry(&'static str),
    /// The store directory contains too many final or interrupted entries.
    #[error("Certified-Serve payload directory exceeds its {capacity} entry traversal bound")]
    DirectoryCapacityExceeded {
        /// Maximum entries inspected during one open.
        capacity: usize,
    },
    /// The store directory contains a name outside the closed format.
    #[error("unexpected Certified-Serve payload entry: {}", .0.display())]
    UnexpectedEntry(PathBuf),
    /// A directory entry is a symlink or another non-regular file.
    #[error("Certified-Serve payload entry is not a regular file: {}", .0.display())]
    NonRegularEntry(PathBuf),
    /// A framed payload is malformed, non-canonical, or corrupt.
    #[error("invalid Certified-Serve payload frame {}: {reason}", path.display())]
    InvalidFrame {
        /// Invalid file path.
        path: PathBuf,
        /// Deterministic rejection reason.
        reason: String,
    },
    /// A file decoded correctly but belongs to another immutable height.
    #[error("Certified-Serve payload belongs to another height context: {}", .0.display())]
    ForeignContext(PathBuf),
    /// A file name does not equal its decoded exact request hash.
    #[error("Certified-Serve payload filename does not match its request hash: {}", .0.display())]
    RequestHashFilenameMismatch(PathBuf),
    /// Two files decoded to the same exact request hash.
    #[error("duplicate Certified-Serve payload request hash")]
    DuplicateRequestHash,
    /// The authenticated wrapper disagreed with the canonical request hash.
    #[error("authenticated Certified-Serve request hash mismatch")]
    AuthenticatedRequestHashMismatch,
    /// Pending persistence was authorized by a different verified height.
    #[error("Certified-Serve pending authority belongs to another verified height context")]
    ForeignVerifiedContext,
    /// Independent request/QC authentication failed under the verified height.
    #[error("Certified-Serve pending request failed verified authentication: {0}")]
    InvalidAuthenticatedRequest(String),
    /// The claimed local validator is absent from the roster or did not sign
    /// the request's retention certificate.
    #[error("local validator has no certified retention authority for this Serve request")]
    LocalRetentionAuthorityAbsent,
    /// Completed persistence used a body store from another immutable height.
    #[error("Certified-Serve completion used a foreign durable body store")]
    ForeignBodyStore,
    /// The supplied durable body receipt does not identify the exact response
    /// manifest, request round, and request subject.
    #[error("Certified-Serve completion used a mismatched durable body receipt")]
    DurableBodyReceiptMismatch,
    /// Reloading the exact body represented by the durable receipt failed.
    #[error("Certified-Serve completion could not reload its durable body: {0}")]
    InvalidDurableBody(String),
    /// Response bytes differ from the canonical bytes protected by the exact
    /// durable body receipt.
    #[error("Certified-Serve response differs from its exact durable body")]
    DurableResponseBodyMismatch,
    /// The hard per-height payload capacity is exhausted.
    #[error("Certified-Serve payload capacity {capacity} is exhausted")]
    PayloadCapacityExceeded {
        /// Immutable maximum number of exact request files.
        capacity: usize,
    },
    /// A terminal transition named an unknown exact request.
    #[error("unknown Certified-Serve payload request")]
    UnknownPayload,
    /// A request hash identified different canonical request bytes.
    #[error("Certified-Serve request hash collision")]
    RequestHashCollision,
    /// A compensating deletion no longer named the exact pending frame.
    #[error("Certified-Serve pending rollback no longer matches its durable frame")]
    PendingRollbackMismatch,
    /// Startup cleanup used a stale, incomplete, or foreign authenticated cut.
    #[error("Certified-Serve authenticated recovery cut does not match the open store")]
    AuthenticatedRecoveryCutMismatch,
    /// A terminal payload had no lifecycle-ledger owner.
    #[error("terminal Certified-Serve payload has no lifecycle-ledger owner")]
    OrphanTerminalPayload,
    /// A pending admission attempted to resurrect a terminal request.
    #[error("terminal Certified-Serve payload cannot return to pending")]
    TerminalResurrection,
    /// A terminal state differs from an already durable terminal result.
    #[error("conflicting Certified-Serve terminal payload")]
    TerminalConflict,
    /// A canonical frame exceeds its context-derived byte bound.
    #[error("Certified-Serve payload frame uses {actual} bytes, exceeding bound {bound}")]
    EntryTooLarge {
        /// Actual encoded frame size.
        actual: u64,
        /// Immutable maximum frame size.
        bound: u64,
    },
}

/// Crash-safe owner of all exact Certified-Serve payload files for one height.
#[derive(Debug)]
pub(crate) struct CertifiedServePayloadStoreV1 {
    directory: PathBuf,
    context: wire::HeightContext,
    max_entries: usize,
    max_entry_bytes: u64,
    indexed: BTreeSet<CertifiedServePayloadId>,
}

impl CertifiedServePayloadStoreV1 {
    /// Open one immutable height store and return its move-only recovery cut.
    ///
    /// Regular interrupted temporary files are discarded before the cut is
    /// returned. Symlinks, unknown names, foreign contexts, oversized files,
    /// and non-canonical frames fail closed.
    ///
    /// # Errors
    ///
    /// Returns an error when geometry cannot be derived or any directory entry
    /// fails the closed storage contract.
    pub(crate) fn open(
        root: &Path,
        context: &wire::HeightContext,
    ) -> Result<(Self, CertifiedServePayloadRecoveryCut), CertifiedServePayloadStoreError> {
        Self::open_with_max_entries(root, context, MAX_CERTIFIED_SERVE_PAYLOADS_PER_HEIGHT)
    }

    fn open_with_max_entries(
        root: &Path,
        context: &wire::HeightContext,
        max_entries: usize,
    ) -> Result<(Self, CertifiedServePayloadRecoveryCut), CertifiedServePayloadStoreError> {
        if max_entries == 0 || max_entries > MAX_CERTIFIED_SERVE_PAYLOADS_PER_HEIGHT {
            return Err(CertifiedServePayloadStoreError::InvalidGeometry(
                "payload count is zero or exceeds the per-height hard bound",
            ));
        }
        context
            .validate()
            .map_err(|error| CertifiedServePayloadStoreError::InvalidFrame {
                path: root.to_path_buf(),
                reason: format!("invalid height context: {error}"),
            })?;
        let max_entry_bytes = derive_max_entry_bytes(context)?;
        let directory = root.join(STORE_DIRECTORY);
        ensure_durable_directory(&directory)?;

        let mut store = Self {
            directory,
            context: context.clone(),
            max_entries,
            max_entry_bytes,
            indexed: BTreeSet::new(),
        };
        let traversal_capacity =
            max_entries
                .checked_mul(2)
                .ok_or(CertifiedServePayloadStoreError::InvalidGeometry(
                    "directory traversal capacity overflowed",
                ))?;
        let mut payloads = BTreeMap::new();
        let mut traversed = 0_usize;
        let mut removed_temporary = false;
        let entries = fs::read_dir(&store.directory)
            .map_err(|source| io_error("read directory", &store.directory, source))?;
        for entry in entries {
            let entry = entry
                .map_err(|source| io_error("read directory entry", &store.directory, source))?;
            traversed = traversed.checked_add(1).ok_or(
                CertifiedServePayloadStoreError::DirectoryCapacityExceeded {
                    capacity: traversal_capacity,
                },
            )?;
            if traversed > traversal_capacity {
                return Err(CertifiedServePayloadStoreError::DirectoryCapacityExceeded {
                    capacity: traversal_capacity,
                });
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|source| io_error("inspect entry", &path, source))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(CertifiedServePayloadStoreError::NonRegularEntry(path));
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                return Err(CertifiedServePayloadStoreError::UnexpectedEntry(path));
            };
            if has_canonical_hash_name(name, TEMPORARY_FILE_SUFFIX) {
                fs::remove_file(&path)
                    .map_err(|source| io_error("discard interrupted file", &path, source))?;
                removed_temporary = true;
                continue;
            }
            if !has_canonical_hash_name(name, FILE_SUFFIX) {
                return Err(CertifiedServePayloadStoreError::UnexpectedEntry(path));
            }
            if payloads.len() >= max_entries {
                return Err(CertifiedServePayloadStoreError::PayloadCapacityExceeded {
                    capacity: max_entries,
                });
            }
            let payload = store.load_path(&path, metadata.len())?;
            if store.path_for(payload.id()) != path {
                return Err(CertifiedServePayloadStoreError::RequestHashFilenameMismatch(path));
            }
            if payloads.insert(payload.id(), payload).is_some() {
                return Err(CertifiedServePayloadStoreError::DuplicateRequestHash);
            }
        }
        if removed_temporary {
            sync_directory(&store.directory)?;
        }
        store.indexed.extend(payloads.keys().copied());
        let recovery = CertifiedServePayloadRecoveryCut {
            context_id: context.id(),
            height: context.height,
            payloads,
        };
        Ok((store, recovery))
    }

    /// Test the production retention path while requiring a Pending result.
    ///
    /// Exact pending repeats are idempotent. A terminal exact request cannot be
    /// resurrected. The returned receipt is minted only after file and
    /// directory synchronisation succeeds. Before writing, this method
    /// independently reauthenticates the request under `verified` and proves
    /// that the validator owning `local_signer` belongs to the certificate
    /// signer set.
    ///
    /// # Errors
    ///
    /// Returns an error for foreign verification authority, absent local
    /// retention authority, a hash disagreement, capacity exhaustion,
    /// collision, terminal resurrection, or filesystem failure.
    #[cfg(test)]
    pub(crate) fn persist_pending_with_verified_retention(
        &mut self,
        verified: &VerifiedHeightContext,
        local_signer: &KeyPair,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> Result<DurableCertifiedServeAdmissionReceipt, CertifiedServePayloadStoreError> {
        let publication = self.retain_for_admission_with_verified_retention(
            verified,
            local_signer,
            authenticated,
        )?;
        publication
            .is_pending()
            .then_some(publication.receipt())
            .ok_or(CertifiedServePayloadStoreError::TerminalResurrection)
    }

    /// Retain an exact verified request for fresh admission or tombstone replay.
    ///
    /// Existing terminal material is never rewritten. Its sealed publication
    /// may only be consumed by a coordinator which already owns the matching
    /// durable lifecycle row.
    pub(super) fn retain_for_admission_with_verified_retention(
        &mut self,
        verified: &VerifiedHeightContext,
        local_signer: &KeyPair,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> Result<DurableCertifiedServeAdmissionPublication, CertifiedServePayloadStoreError> {
        let local_validator =
            self.verified_local_retainer(verified, local_signer, authenticated)?;
        self.retain_for_admission_inner(authenticated, local_validator)
    }

    fn verified_local_retainer(
        &self,
        verified: &VerifiedHeightContext,
        local_signer: &KeyPair,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> Result<wire::ValidatorIndex, CertifiedServePayloadStoreError> {
        if verified.context() != &self.context {
            return Err(CertifiedServePayloadStoreError::ForeignVerifiedContext);
        }
        let request = authenticated.request();
        let requester = request.requester.clone();
        let independently_authenticated = authenticate_certified_body_request_with_verified_height(
            verified,
            request.clone(),
            &requester,
        )
        .map_err(|error| {
            CertifiedServePayloadStoreError::InvalidAuthenticatedRequest(error.to_string())
        })?;
        if &independently_authenticated != authenticated {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRequestHashMismatch);
        }
        let local_validator = self
            .context
            .roster
            .iter()
            .position(|entry| entry.validator.public_key() == local_signer.public_key())
            .and_then(|index| wire::ValidatorIndex::try_from(index).ok())
            .ok_or(CertifiedServePayloadStoreError::LocalRetentionAuthorityAbsent)?;
        if request
            .certificate
            .signers
            .binary_search(&local_validator)
            .is_err()
        {
            return Err(CertifiedServePayloadStoreError::LocalRetentionAuthorityAbsent);
        }
        Ok(local_validator)
    }

    /// Test-only persistence helper for synthetic non-cryptographic fixtures.
    /// Production code cannot mint a pending receipt through this path.
    #[cfg(test)]
    pub(crate) fn persist_pending(
        &mut self,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> Result<DurableCertifiedServeAdmissionReceipt, CertifiedServePayloadStoreError> {
        let local_retainer = authenticated
            .request()
            .certificate
            .signers
            .first()
            .copied()
            .unwrap_or(0);
        let publication = self.retain_for_admission_inner(authenticated, local_retainer)?;
        publication
            .is_pending()
            .then_some(publication.receipt())
            .ok_or(CertifiedServePayloadStoreError::TerminalResurrection)
    }

    fn retain_for_admission_inner(
        &mut self,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        local_retainer: wire::ValidatorIndex,
    ) -> Result<DurableCertifiedServeAdmissionPublication, CertifiedServePayloadStoreError> {
        let request = authenticated.request();
        let id = CertifiedServePayloadId::from_request(request);
        if id.request_hash() != authenticated.request_hash() {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRequestHashMismatch);
        }
        self.validate_request(request, &self.directory)?;
        if self.indexed.contains(&id) {
            let existing = self.load_id(id)?;
            if existing.request != *request {
                return Err(CertifiedServePayloadStoreError::RequestHashCollision);
            }
            return Ok(DurableCertifiedServeAdmissionPublication {
                receipt: admission_receipt(&existing, local_retainer),
                pending: matches!(
                    existing.state,
                    PersistedCertifiedServePayloadStateV1::Pending
                ),
            });
        }
        if self.indexed.len() >= self.max_entries {
            return Err(CertifiedServePayloadStoreError::PayloadCapacityExceeded {
                capacity: self.max_entries,
            });
        }
        let payload = PersistedCertifiedServePayloadV1 {
            format_version: FORMAT_VERSION,
            context_id: self.context.id(),
            height: self.context.height,
            request_hash: id.request_hash(),
            request: request.clone(),
            state: PersistedCertifiedServePayloadStateV1::Pending,
        };
        let receipt = admission_receipt(&payload, local_retainer);
        self.persist_payload(&payload)?;
        self.indexed.insert(id);
        Ok(DurableCertifiedServeAdmissionPublication {
            receipt,
            pending: true,
        })
    }

    /// Remove an exact pending publication after admission conclusively
    /// declined it.
    ///
    /// This is the compensating half of the payload-first/ledger-second
    /// admission transaction. The sealed receipt must still name the exact
    /// pending frame; terminal material is never removed through this path.
    /// The directory deletion is synchronised before the in-memory index is
    /// released.
    pub(super) fn rollback_pending(
        &mut self,
        receipt: DurableCertifiedServeAdmissionReceipt,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        self.rollback_pending_batch(&[receipt])
    }

    /// Remove one exact batch of pending publications at a typed rollover cut.
    ///
    /// Every receipt is reloaded and validated before the first unlink. A
    /// partial filesystem failure therefore requires restart, but can never
    /// remove terminal evidence or a publication outside the supplied batch.
    pub(super) fn rollback_pending_batch(
        &mut self,
        receipts: &[DurableCertifiedServeAdmissionReceipt],
    ) -> Result<(), CertifiedServePayloadStoreError> {
        let mut ids = BTreeSet::new();
        for receipt in receipts {
            let id = receipt.id();
            if !ids.insert(id) || !self.indexed.contains(&id) {
                return Err(CertifiedServePayloadStoreError::PendingRollbackMismatch);
            }
            let payload = self.load_id(id)?;
            if !matches!(
                payload.state,
                PersistedCertifiedServePayloadStateV1::Pending
            ) || payload.payload_hash() != receipt.payload_hash()
                || HashOf::new(&payload.request.certificate) != receipt.certificate_hash()
            {
                return Err(CertifiedServePayloadStoreError::PendingRollbackMismatch);
            }
        }
        for id in &ids {
            let path = self.path_for(*id);
            fs::remove_file(&path)
                .map_err(|source| io_error("roll back pending publication", &path, source))?;
        }
        if !ids.is_empty() {
            sync_directory(&self.directory)?;
            for id in ids {
                let removed = self.indexed.remove(&id);
                debug_assert!(removed, "validated pending publication remained indexed");
            }
        }
        Ok(())
    }

    /// Prune every fully authenticated payload that has no lifecycle-ledger
    /// owner after restart reconciliation succeeds.
    ///
    /// The authenticated cut must cover the store's complete current index and
    /// every retained identity must occur in that cut. Each frame is reloaded
    /// and matched to the cut before any deletion, preventing a stale recovery
    /// snapshot from deleting newly published work.
    pub(super) fn prune_authenticated_orphans(
        &mut self,
        authenticated: &AuthenticatedCertifiedServePayloadRecoveryCut,
        retained: &BTreeSet<CertifiedServePayloadId>,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        self.validate_authenticated_cut(authenticated)?;
        let cut_ids = authenticated
            .iter()
            .map(AuthenticatedRecoveredCertifiedServePayload::id)
            .collect::<BTreeSet<_>>();
        if !retained.is_subset(&cut_ids) {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch);
        }

        let orphans = self
            .indexed
            .difference(retained)
            .copied()
            .collect::<BTreeSet<_>>();
        if authenticated.iter().any(|payload| {
            orphans.contains(&payload.id())
                && !matches!(
                    payload.state(),
                    AuthenticatedRecoveredCertifiedServePayloadState::Pending
                )
        }) {
            return Err(CertifiedServePayloadStoreError::OrphanTerminalPayload);
        }
        for id in &orphans {
            let path = self.path_for(*id);
            fs::remove_file(&path)
                .map_err(|source| io_error("prune orphaned publication", &path, source))?;
        }
        if !orphans.is_empty() {
            sync_directory(&self.directory)?;
            for id in orphans {
                let removed = self.indexed.remove(&id);
                debug_assert!(removed, "authenticated orphan remained indexed");
            }
        }
        Ok(())
    }

    /// Verify that a post-authentication startup cut still covers the complete
    /// open store byte-for-byte.
    pub(super) fn validate_authenticated_cut(
        &self,
        authenticated: &AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        if authenticated.context_id() != self.context.id()
            || authenticated.height() != self.context.height
        {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch);
        }
        let cut_ids = authenticated
            .iter()
            .map(AuthenticatedRecoveredCertifiedServePayload::id)
            .collect::<BTreeSet<_>>();
        if cut_ids != self.indexed {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch);
        }
        for recovered in authenticated.iter() {
            let payload = self.load_id(recovered.id())?;
            if payload.payload_hash() != recovered.payload_hash() {
                return Err(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch);
            }
        }
        Ok(())
    }

    /// Persist terminal metadata from one locally authenticated response.
    ///
    /// The exact response body must already be durable in `body_store` under
    /// `durable_body`. This store deliberately persists no body bytes. Exact
    /// terminal repeats are idempotent.
    ///
    /// # Errors
    ///
    /// Returns an error when the authenticated request is unknown, the response
    /// changes its request binding or certified local retention authority, its
    /// responder signature is invalid, the receipt/store cannot reproduce the
    /// exact response body, another terminal result already exists, or
    /// persistence fails.
    #[cfg(not(test))]
    pub(crate) fn persist_completed(
        &mut self,
        authenticated_request: &AuthenticatedCertifiedBodyRequest,
        durable_body: &DurableBodyReceipt,
        body_store: &V2BodyStore,
        response: &wire::CertifiedBodyResponse,
    ) -> Result<DurableCertifiedServeCompletedReceipt, CertifiedServePayloadStoreError> {
        self.persist_completed_with_durable_body(
            authenticated_request,
            durable_body,
            body_store,
            response,
        )
    }

    /// Test-only convenience for synthetic responses that have no canonical
    /// `SignedBlockWire` body. Production completion must provide the exact
    /// body-store receipt through the signature above.
    #[cfg(test)]
    pub(crate) fn persist_completed(
        &mut self,
        authenticated_request: &AuthenticatedCertifiedBodyRequest,
        response: &wire::CertifiedBodyResponse,
    ) -> Result<DurableCertifiedServeCompletedReceipt, CertifiedServePayloadStoreError> {
        if authenticated_request.request_hash() != response.request_hash {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRequestHashMismatch);
        }
        self.persist_completed_response(authenticated_request.request(), response)
    }

    #[cfg(any(not(test), feature = "bls"))]
    fn persist_completed_with_durable_body(
        &mut self,
        authenticated_request: &AuthenticatedCertifiedBodyRequest,
        durable_body: &DurableBodyReceipt,
        body_store: &V2BodyStore,
        response: &wire::CertifiedBodyResponse,
    ) -> Result<DurableCertifiedServeCompletedReceipt, CertifiedServePayloadStoreError> {
        if authenticated_request.request_hash() != response.request_hash {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRequestHashMismatch);
        }
        self.validate_durable_response_body(
            authenticated_request.request(),
            durable_body,
            body_store,
            response,
        )?;
        self.persist_completed_response(authenticated_request.request(), response)
    }

    fn persist_completed_response(
        &mut self,
        authenticated_request: &wire::CertifiedBodyRequest,
        response: &wire::CertifiedBodyResponse,
    ) -> Result<DurableCertifiedServeCompletedReceipt, CertifiedServePayloadStoreError> {
        let id = CertifiedServePayloadId(response.request_hash);
        if !self.indexed.contains(&id) {
            return Err(CertifiedServePayloadStoreError::UnknownPayload);
        }
        let mut payload = self.load_id(id)?;
        if payload.request != *authenticated_request {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRequestHashMismatch);
        }
        self.validate_completed_response(&payload.request, response)?;
        let completed = PersistedCertifiedServePayloadStateV1::Completed {
            response_hash: HashOf::new(response),
            manifest: response.manifest.clone(),
            responder: response.responder,
            signature: response.signature.clone(),
        };
        match &payload.state {
            PersistedCertifiedServePayloadStateV1::Pending => {}
            existing @ PersistedCertifiedServePayloadStateV1::Completed { .. }
                if existing == &completed =>
            {
                return completed_receipt(&payload);
            }
            PersistedCertifiedServePayloadStateV1::Completed { .. }
            | PersistedCertifiedServePayloadStateV1::Negative { .. } => {
                return Err(CertifiedServePayloadStoreError::TerminalConflict);
            }
        }
        payload.state = completed;
        let receipt = completed_receipt(&payload)?;
        self.persist_payload(&payload)?;
        Ok(receipt)
    }

    /// Persist one deterministic negative terminal result.
    ///
    /// Exact repeats are idempotent; neither a different negative tag nor a
    /// completed response can replace a durable terminal result.
    ///
    /// # Errors
    ///
    /// Returns an error when `id` is unknown, a conflicting terminal result
    /// exists, or persistence fails.
    pub(crate) fn persist_negative(
        &mut self,
        id: CertifiedServePayloadId,
        outcome: CertifiedServePayloadNegativeOutcome,
    ) -> Result<DurableCertifiedServeNegativeReceipt, CertifiedServePayloadStoreError> {
        if !self.indexed.contains(&id) {
            return Err(CertifiedServePayloadStoreError::UnknownPayload);
        }
        let mut payload = self.load_id(id)?;
        let negative = PersistedCertifiedServePayloadStateV1::Negative { outcome };
        match &payload.state {
            PersistedCertifiedServePayloadStateV1::Pending => {}
            existing @ PersistedCertifiedServePayloadStateV1::Negative { .. }
                if existing == &negative =>
            {
                return negative_receipt(&payload);
            }
            PersistedCertifiedServePayloadStateV1::Completed { .. }
            | PersistedCertifiedServePayloadStateV1::Negative { .. } => {
                return Err(CertifiedServePayloadStoreError::TerminalConflict);
            }
        }
        payload.state = negative;
        let receipt = negative_receipt(&payload)?;
        self.persist_payload(&payload)?;
        Ok(receipt)
    }

    fn validate_request(
        &self,
        request: &wire::CertifiedBodyRequest,
        path: &Path,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        request
            .validate(&self.context)
            .map_err(|error| invalid_frame(path, format!("invalid retained request: {error}")))?;
        authenticate_certified_body_request_identity(request, &request.requester).map_err(
            |error| invalid_frame(path, format!("unauthenticated retained requester: {error}")),
        )?;
        Ok(())
    }

    #[cfg(any(not(test), feature = "bls"))]
    fn validate_durable_response_body(
        &self,
        request: &wire::CertifiedBodyRequest,
        durable_body: &DurableBodyReceipt,
        body_store: &V2BodyStore,
        response: &wire::CertifiedBodyResponse,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        if !body_store.matches_context(&self.context) {
            return Err(CertifiedServePayloadStoreError::ForeignBodyStore);
        }
        if durable_body.context_id() != self.context.id()
            || durable_body.round() != request.round
            || durable_body.subject() != request.subject
            || response.manifest.round != request.round
            || response.manifest.subject != request.subject
            || durable_body.manifest_hash() != HashOf::new(&response.manifest)
        {
            return Err(CertifiedServePayloadStoreError::DurableBodyReceiptMismatch);
        }
        let canonical_body = body_store
            .load_canonical_wire(durable_body)
            .map_err(|error| {
                CertifiedServePayloadStoreError::InvalidDurableBody(error.to_string())
            })?;
        if canonical_body != response.body {
            return Err(CertifiedServePayloadStoreError::DurableResponseBodyMismatch);
        }
        Ok(())
    }

    fn validate_completed_response(
        &self,
        request: &wire::CertifiedBodyRequest,
        response: &wire::CertifiedBodyResponse,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        response.validate(&self.context).map_err(|error| {
            invalid_frame(&self.directory, format!("invalid response: {error}"))
        })?;
        if response.request_hash != HashOf::new(request)
            || response.manifest.round != request.round
            || response.manifest.subject != request.subject
        {
            return Err(invalid_frame(
                &self.directory,
                "response changed its exact request binding",
            ));
        }
        let responder_index = usize::try_from(response.responder)
            .ok()
            .filter(|index| *index < self.context.roster.len())
            .ok_or_else(|| {
                invalid_frame(
                    &self.directory,
                    "response signer is outside the frozen roster",
                )
            })?;
        if request
            .certificate
            .signers
            .binary_search(&response.responder)
            .is_err()
        {
            return Err(invalid_frame(
                &self.directory,
                "response signer lost certified local retention authority",
            ));
        }
        let responder = &self.context.roster[responder_index].validator;
        response
            .validate_against(&self.context, request, responder)
            .map_err(|error| {
                invalid_frame(
                    &self.directory,
                    format!("response failed exact request validation: {error}"),
                )
            })?;
        let signature = Signature::try_from_bytes(&response.signature).map_err(|error| {
            invalid_frame(
                &self.directory,
                format!("response signature is malformed: {error}"),
            )
        })?;
        signature
            .verify(responder.public_key(), &response.signature_preimage())
            .map_err(|error| {
                invalid_frame(
                    &self.directory,
                    format!("response signature is invalid: {error}"),
                )
            })?;
        Ok(())
    }

    fn validate_recovered_payload(
        &self,
        payload: &PersistedCertifiedServePayloadV1,
        path: &Path,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        if payload.format_version != FORMAT_VERSION {
            return Err(invalid_frame(
                path,
                format!(
                    "payload uses unsupported version {}",
                    payload.format_version
                ),
            ));
        }
        if payload.context_id != self.context.id() || payload.height != self.context.height {
            return Err(CertifiedServePayloadStoreError::ForeignContext(
                path.to_path_buf(),
            ));
        }
        if payload.request_hash != HashOf::new(&payload.request) {
            return Err(invalid_frame(path, "retained request hash mismatch"));
        }
        self.validate_request(&payload.request, path)?;
        match &payload.state {
            PersistedCertifiedServePayloadStateV1::Pending
            | PersistedCertifiedServePayloadStateV1::Negative { .. } => {}
            PersistedCertifiedServePayloadStateV1::Completed {
                manifest,
                responder,
                signature,
                ..
            } => {
                manifest.validate(&self.context).map_err(|error| {
                    invalid_frame(path, format!("invalid retained response manifest: {error}"))
                })?;
                if manifest.round != payload.request.round
                    || manifest.subject != payload.request.subject
                {
                    return Err(invalid_frame(
                        path,
                        "retained response manifest changed its request binding",
                    ));
                }
                if usize::try_from(*responder)
                    .ok()
                    .is_none_or(|index| index >= self.context.roster.len())
                    || signature.is_empty()
                    || signature.len() > wire::MAX_CONSENSUS_SIGNATURE_BYTES
                {
                    return Err(invalid_frame(
                        path,
                        "retained response signer metadata is invalid",
                    ));
                }
            }
        }
        Ok(())
    }

    fn load_id(
        &self,
        id: CertifiedServePayloadId,
    ) -> Result<PersistedCertifiedServePayloadV1, CertifiedServePayloadStoreError> {
        let path = self.path_for(id);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|source| io_error("inspect indexed file", &path, source))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(CertifiedServePayloadStoreError::NonRegularEntry(path));
        }
        let payload = self.load_path(&path, metadata.len())?;
        if payload.id() != id {
            return Err(CertifiedServePayloadStoreError::RequestHashFilenameMismatch(path));
        }
        Ok(payload)
    }

    fn load_path(
        &self,
        path: &Path,
        metadata_len: u64,
    ) -> Result<PersistedCertifiedServePayloadV1, CertifiedServePayloadStoreError> {
        if metadata_len > self.max_entry_bytes {
            return Err(CertifiedServePayloadStoreError::EntryTooLarge {
                actual: metadata_len,
                bound: self.max_entry_bytes,
            });
        }
        let read_limit = self.max_entry_bytes.checked_add(1).ok_or(
            CertifiedServePayloadStoreError::InvalidGeometry("file read bound overflowed"),
        )?;
        let mut bytes = Vec::new();
        File::open(path)
            .map_err(|source| io_error("open file", path, source))?
            .take(read_limit)
            .read_to_end(&mut bytes)
            .map_err(|source| io_error("read file", path, source))?;
        let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if actual > self.max_entry_bytes {
            return Err(CertifiedServePayloadStoreError::EntryTooLarge {
                actual,
                bound: self.max_entry_bytes,
            });
        }
        let payload = decode_frame(&bytes, self.max_entry_bytes, path)?;
        self.validate_recovered_payload(&payload, path)?;
        Ok(payload)
    }

    fn persist_payload(
        &self,
        payload: &PersistedCertifiedServePayloadV1,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        let path = self.path_for(payload.id());
        self.validate_recovered_payload(payload, &path)?;
        let (frame, _) = encode_frame(payload, self.max_entry_bytes)?;
        let temporary = self.temporary_path(payload.id());
        match fs::symlink_metadata(&temporary) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(CertifiedServePayloadStoreError::NonRegularEntry(temporary));
            }
            Ok(_) => {
                fs::remove_file(&temporary)
                    .map_err(|source| io_error("discard interrupted file", &temporary, source))?;
                sync_directory(&self.directory)?;
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(source) => return Err(io_error("inspect temporary file", &temporary, source)),
        }
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|source| io_error("create temporary file", &temporary, source))?;
        file.write_all(&frame)
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all())
            .map_err(|source| io_error("synchronise temporary file", &temporary, source))?;
        drop(file);
        fs::rename(&temporary, &path).map_err(|source| io_error("publish file", &path, source))?;
        sync_directory(&self.directory)?;
        Ok(())
    }

    fn path_for(&self, id: CertifiedServePayloadId) -> PathBuf {
        self.directory.join(format!(
            "{}{}",
            hex::encode(id.request_hash().as_ref()),
            FILE_SUFFIX
        ))
    }

    fn temporary_path(&self, id: CertifiedServePayloadId) -> PathBuf {
        self.directory.join(format!(
            "{}{}",
            hex::encode(id.request_hash().as_ref()),
            TEMPORARY_FILE_SUFFIX
        ))
    }
}

fn admission_receipt(
    payload: &PersistedCertifiedServePayloadV1,
    local_retainer: wire::ValidatorIndex,
) -> DurableCertifiedServeAdmissionReceipt {
    DurableCertifiedServeAdmissionReceipt {
        id: payload.id(),
        certificate_hash: HashOf::new(&payload.request.certificate),
        payload_hash: payload.payload_hash(),
        local_retainer,
    }
}

fn completed_receipt(
    payload: &PersistedCertifiedServePayloadV1,
) -> Result<DurableCertifiedServeCompletedReceipt, CertifiedServePayloadStoreError> {
    let PersistedCertifiedServePayloadStateV1::Completed { response_hash, .. } = &payload.state
    else {
        return Err(CertifiedServePayloadStoreError::TerminalConflict);
    };
    Ok(DurableCertifiedServeCompletedReceipt {
        id: payload.id(),
        certificate_hash: HashOf::new(&payload.request.certificate),
        payload_hash: payload.payload_hash(),
        response_hash: *response_hash,
    })
}

fn negative_receipt(
    payload: &PersistedCertifiedServePayloadV1,
) -> Result<DurableCertifiedServeNegativeReceipt, CertifiedServePayloadStoreError> {
    let PersistedCertifiedServePayloadStateV1::Negative { outcome } = &payload.state else {
        return Err(CertifiedServePayloadStoreError::TerminalConflict);
    };
    Ok(DurableCertifiedServeNegativeReceipt {
        id: payload.id(),
        certificate_hash: HashOf::new(&payload.request.certificate),
        payload_hash: payload.payload_hash(),
        outcome: *outcome,
    })
}

fn derive_max_entry_bytes(
    context: &wire::HeightContext,
) -> Result<u64, CertifiedServePayloadStoreError> {
    let roster_len = u64::try_from(context.roster.len()).map_err(|_| {
        CertifiedServePayloadStoreError::InvalidGeometry("roster length is not representable")
    })?;
    let signature_record_bytes = u64::try_from(wire::MAX_CONSENSUS_SIGNATURE_BYTES + Hash::LENGTH)
        .map_err(|_| {
            CertifiedServePayloadStoreError::InvalidGeometry(
                "signature record bound is not representable",
            )
        })?;
    let roster_bytes = roster_len.checked_mul(signature_record_bytes).ok_or(
        CertifiedServePayloadStoreError::InvalidGeometry("roster byte bound overflowed"),
    )?;
    let manifest_hash_bytes = u64::from(context.da_layout.max_chunk_count)
        .checked_mul(u64::try_from(Hash::LENGTH).expect("hash length fits u64"))
        .ok_or(CertifiedServePayloadStoreError::InvalidGeometry(
            "manifest hash byte bound overflowed",
        ))?;
    ENTRY_FIXED_HEADROOM_BYTES
        .checked_add(roster_bytes)
        .and_then(|bytes| bytes.checked_add(manifest_hash_bytes))
        .and_then(|bytes| {
            bytes.checked_add(
                u64::try_from(FRAME_HEADER_BYTES).expect("frame header length fits u64"),
            )
        })
        .ok_or(CertifiedServePayloadStoreError::InvalidGeometry(
            "entry byte bound overflowed",
        ))
}

fn encode_frame(
    payload: &PersistedCertifiedServePayloadV1,
    max_entry_bytes: u64,
) -> Result<(Vec<u8>, Hash), CertifiedServePayloadStoreError> {
    let encoded = payload.encode();
    let encoded_len = u64::try_from(encoded.len()).map_err(|_| {
        CertifiedServePayloadStoreError::InvalidGeometry("payload length is not representable")
    })?;
    let frame_len = u64::try_from(FRAME_HEADER_BYTES)
        .expect("frame header length fits u64")
        .checked_add(encoded_len)
        .ok_or(CertifiedServePayloadStoreError::InvalidGeometry(
            "frame length overflowed",
        ))?;
    if frame_len > max_entry_bytes {
        return Err(CertifiedServePayloadStoreError::EntryTooLarge {
            actual: frame_len,
            bound: max_entry_bytes,
        });
    }
    let capacity = usize::try_from(frame_len).map_err(|_| {
        CertifiedServePayloadStoreError::InvalidGeometry("frame is not addressable")
    })?;
    let digest = Hash::new(&encoded);
    let mut frame = Vec::with_capacity(capacity);
    frame.extend_from_slice(FRAME_MAGIC);
    frame.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    frame.extend_from_slice(&encoded_len.to_le_bytes());
    frame.extend_from_slice(digest.as_ref());
    frame.extend_from_slice(&encoded);
    Ok((frame, digest))
}

fn decode_frame(
    frame: &[u8],
    max_entry_bytes: u64,
    path: &Path,
) -> Result<PersistedCertifiedServePayloadV1, CertifiedServePayloadStoreError> {
    let actual = u64::try_from(frame.len()).unwrap_or(u64::MAX);
    if actual > max_entry_bytes {
        return Err(CertifiedServePayloadStoreError::EntryTooLarge {
            actual,
            bound: max_entry_bytes,
        });
    }
    if frame.len() < FRAME_HEADER_BYTES
        || frame.get(..FRAME_MAGIC.len()) != Some(FRAME_MAGIC.as_slice())
    {
        return Err(invalid_frame(
            path,
            "invalid frame magic or truncated header",
        ));
    }
    let version_offset = FRAME_MAGIC.len();
    let version = u16::from_le_bytes(
        frame[version_offset..version_offset + size_of::<u16>()]
            .try_into()
            .map_err(|_| invalid_frame(path, "truncated frame version"))?,
    );
    if version != FORMAT_VERSION {
        return Err(invalid_frame(
            path,
            format!("unsupported frame version {version}"),
        ));
    }
    let length_offset = version_offset + size_of::<u16>();
    let encoded_len = u64::from_le_bytes(
        frame[length_offset..length_offset + size_of::<u64>()]
            .try_into()
            .map_err(|_| invalid_frame(path, "truncated payload length"))?,
    );
    let encoded_len = usize::try_from(encoded_len)
        .map_err(|_| invalid_frame(path, "payload length is not addressable"))?;
    let checksum_offset = length_offset + size_of::<u64>();
    let payload_offset = checksum_offset + CHECKSUM_BYTES;
    if payload_offset.checked_add(encoded_len) != Some(frame.len()) {
        return Err(invalid_frame(path, "frame length is inconsistent"));
    }
    let encoded = &frame[payload_offset..];
    if Hash::new(encoded).as_ref() != &frame[checksum_offset..payload_offset] {
        return Err(invalid_frame(path, "frame checksum mismatch"));
    }
    let mut cursor = encoded;
    let payload = PersistedCertifiedServePayloadV1::decode_all(&mut cursor)
        .map_err(|error| invalid_frame(path, format!("Norito decode failed: {error}")))?;
    if payload.encode() != encoded {
        return Err(invalid_frame(path, "payload is not canonically encoded"));
    }
    Ok(payload)
}

fn has_canonical_hash_name(name: &str, suffix: &str) -> bool {
    let Some(hash) = name.strip_suffix(suffix) else {
        return false;
    };
    hash.len() == Hash::LENGTH * 2
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid_frame(path: &Path, reason: impl Into<String>) -> CertifiedServePayloadStoreError {
    CertifiedServePayloadStoreError::InvalidFrame {
        path: path.to_path_buf(),
        reason: reason.into(),
    }
}

fn io_error(
    operation: &'static str,
    path: &Path,
    source: std::io::Error,
) -> CertifiedServePayloadStoreError {
    CertifiedServePayloadStoreError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

fn sync_directory(directory: &Path) -> Result<(), CertifiedServePayloadStoreError> {
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|source| io_error("synchronise directory", directory, source))
}

fn ensure_durable_directory(directory: &Path) -> Result<(), CertifiedServePayloadStoreError> {
    ensure_durable_directory_with(directory, &mut sync_directory)
}

fn ensure_durable_directory_with<Sync>(
    directory: &Path,
    sync: &mut Sync,
) -> Result<(), CertifiedServePayloadStoreError>
where
    Sync: FnMut(&Path) -> Result<(), CertifiedServePayloadStoreError>,
{
    let parent = directory
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    match fs::symlink_metadata(directory) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(CertifiedServePayloadStoreError::NonRegularEntry(
                    directory.to_path_buf(),
                ));
            }
            sync(directory)?;
            if parent != directory {
                sync(parent)?;
            }
            return Ok(());
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(source) => return Err(io_error("inspect directory", directory, source)),
    }

    ensure_durable_directory_with(parent, sync)?;
    match fs::create_dir(directory) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(source) => return Err(io_error("create directory", directory, source)),
    }
    let metadata = fs::symlink_metadata(directory)
        .map_err(|source| io_error("inspect created directory", directory, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CertifiedServePayloadStoreError::NonRegularEntry(
            directory.to_path_buf(),
        ));
    }
    sync(directory)?;
    sync(parent)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "bls")]
    use std::num::NonZeroU64;

    #[cfg(feature = "bls")]
    use iroha_crypto::SignatureOf;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    #[cfg(feature = "bls")]
    use iroha_data_model::block::{BlockSignature, SignedBlock};
    use iroha_data_model::{block::BlockHeader, peer::PeerId};
    use tempfile::TempDir;

    use super::*;
    #[cfg(feature = "bls")]
    use crate::sumeragi::v2_chunks::encode_payload;
    use crate::sumeragi::v2_transport::authenticate_certified_body_request;

    fn context_and_keys() -> (wire::HeightContext, Vec<KeyPair>) {
        let mut keys = (0x41_u8..=0x44)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
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
            network_id: crate::sumeragi::synthetic_network_id("certified-serve-payload-store-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"Serve payload store AMX context"),
            execution_policy_hash: Hash::new(b"Serve payload store execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 8,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 16,
                max_chunk_count: 4,
            },
            leader_seed: [0xA7; 32],
        };
        context.validate().expect("valid fixture context");
        (context, keys)
    }

    #[cfg(feature = "bls")]
    fn verified_bls_context_and_keys() -> (VerifiedHeightContext, Vec<KeyPair>) {
        let mut keys = (0x51_u8..=0x54)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS validator key")
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
            network_id: crate::sumeragi::synthetic_network_id(
                "certified-serve-payload-recovery-test",
            ),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"Serve payload recovery AMX context"),
            execution_policy_hash: Hash::new(b"Serve payload recovery execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1_048_576,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 1_048_576,
                max_chunk_count: 2,
            },
            leader_seed: [0xB7; 32],
        };
        let proofs_of_possession = keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture BLS proof of possession")
            })
            .collect();
        let verified = VerifiedHeightContext::genesis(context, proofs_of_possession)
            .expect("verified BLS height context");
        (verified, keys)
    }

    #[cfg(feature = "bls")]
    fn bls_request(
        verified: &VerifiedHeightContext,
        keys: &[KeyPair],
        valid_certificate: bool,
    ) -> AuthenticatedCertifiedBodyRequest {
        let context = verified.context();
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"Serve authenticated recovery block",
            )),
            payload_hash: Hash::new(b"Serve authenticated recovery body"),
        };
        bls_request_for_subject(verified, keys, valid_certificate, round, subject)
    }

    #[cfg(feature = "bls")]
    fn bls_request_for_subject(
        verified: &VerifiedHeightContext,
        keys: &[KeyPair],
        valid_certificate: bool,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> AuthenticatedCertifiedBodyRequest {
        let context = verified.context();
        let execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"Serve recovery parent state"),
            Hash::new(b"Serve recovery post state"),
            Hash::new(b"Serve recovery ordinary writes"),
            1,
            Hash::new(b"Serve recovery executed block"),
        );
        let signers = vec![0, 1, 2];
        let preimage = wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        }
        .signature_preimage();
        let shares = signers
            .iter()
            .map(|signer| {
                Signature::new(
                    keys[usize::try_from(*signer).expect("small signer")].private_key(),
                    &preimage,
                )
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        let mut aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate fixture QC");
        if !valid_certificate {
            aggregate_signature[0] ^= 0x80;
        }
        let mut request = wire::CertifiedBodyRequest {
            round,
            subject,
            certificate: wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject,
                execution_commitment,
                signers,
                aggregate_signature,
            },
            requester: PeerId::new(keys[3].public_key().clone()),
            signature: Vec::new(),
        };
        request.signature = Signature::new(keys[3].private_key(), &request.signature_preimage())
            .payload()
            .to_vec();
        let requester = request.requester.clone();
        authenticate_certified_body_request(context, request, &requester, |context, certificate| {
            if valid_certificate {
                wire::finality::verify_quorum_certificate_with_validator_pops(
                    context,
                    certificate,
                    verified.proofs_of_possession(),
                )
                .map_err(|error| error.to_string())
            } else {
                Ok(())
            }
        })
        .expect("fixture request authentication policy")
    }

    #[cfg(feature = "bls")]
    fn canonical_body_and_manifest(
        context: &wire::HeightContext,
        keys: &[KeyPair],
        view: wire::View,
    ) -> (Vec<u8>, wire::PayloadManifest) {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view,
        };
        let leader = context.leader(round.view);
        let leader_index = usize::try_from(leader).expect("fixture leader index fits usize");
        let header = BlockHeader::new(
            NonZeroU64::new(round.height).expect("non-zero fixture height"),
            None,
            None,
            None,
            1_000,
            round.view,
        );
        let signature = SignatureOf::try_from_hash(keys[leader_index].private_key(), header.hash())
            .expect("sign fixture block header");
        let block = SignedBlock::presigned(
            BlockSignature::new(u64::from(leader), signature),
            header,
            Vec::new(),
        );
        let body = block.encode_wire().expect("canonical SignedBlockWire");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&body),
        };
        let manifest = encode_payload(context, round, subject, &body)
            .expect("encode canonical fixture payload")
            .manifest()
            .clone();
        (body, manifest)
    }

    #[cfg(feature = "bls")]
    fn signed_certified_response(
        request: &AuthenticatedCertifiedBodyRequest,
        manifest: wire::PayloadManifest,
        body: Vec<u8>,
        responder: wire::ValidatorIndex,
        keys: &[KeyPair],
    ) -> wire::CertifiedBodyResponse {
        let mut response = wire::CertifiedBodyResponse {
            request_hash: request.request_hash(),
            manifest,
            body,
            responder,
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            keys[usize::try_from(responder).expect("fixture responder index")].private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        response
    }

    fn request_and_response(
        context: &wire::HeightContext,
        key: &KeyPair,
        view: u64,
        body: Vec<u8>,
    ) -> (
        AuthenticatedCertifiedBodyRequest,
        wire::CertifiedBodyResponse,
    ) {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view,
        };
        let mut block_hash_preimage = b"Serve payload store block".to_vec();
        block_hash_preimage.extend_from_slice(&view.to_le_bytes());
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                block_hash_preimage,
            )),
            payload_hash: Hash::new(&body),
        };
        let encoded_chunks =
            wire::encode_payload_chunks(context.da_layout, &body).expect("encode fixture payload");
        let manifest = wire::PayloadManifest::derive(
            context,
            round,
            subject,
            u64::try_from(body.len()).expect("fixture body length fits u64"),
            &encoded_chunks,
        )
        .expect("derive fixture manifest");
        let mut request = wire::CertifiedBodyRequest {
            round,
            subject,
            certificate: wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Prepare,
                subject,
                execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                    Hash::new(b"Serve payload store parent state"),
                    Hash::new(b"Serve payload store post state"),
                    Hash::new(b"Serve payload store ordinary writes"),
                    1,
                    Hash::new(b"Serve payload store executed block"),
                ),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xA5; 48],
            },
            requester: PeerId::new(key.public_key().clone()),
            signature: Vec::new(),
        };
        request.signature = Signature::new(key.private_key(), &request.signature_preimage())
            .payload()
            .to_vec();
        let requester = request.requester.clone();
        let authenticated =
            authenticate_certified_body_request(context, request, &requester, |_, _| {
                Ok::<(), &'static str>(())
            })
            .expect("authenticate fixture request");
        let mut response = wire::CertifiedBodyResponse {
            request_hash: authenticated.request_hash(),
            manifest,
            body,
            responder: 0,
            signature: Vec::new(),
        };
        response.signature = Signature::new(key.private_key(), &response.signature_preimage())
            .payload()
            .to_vec();
        (authenticated, response)
    }

    #[test]
    fn pending_and_completed_payload_round_trip_by_signed_request_hash() {
        let temporary = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (request, response) = request_and_response(&context, &keys[0], 0, b"payload!".to_vec());
        let (mut store, recovery) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        assert!(recovery.is_empty());
        assert_eq!(recovery.context_id(), context.id());
        assert_eq!(recovery.height(), context.height);

        let pending = store
            .persist_pending(&request)
            .expect("persist pending request");
        assert_eq!(pending.id().request_hash(), request.request_hash());
        assert_eq!(
            pending.certificate_hash(),
            HashOf::new(&request.request().certificate)
        );
        assert_ne!(pending.payload_hash(), Hash::new([]));
        assert_eq!(
            store.persist_pending(&request).expect("idempotent pending"),
            pending
        );
        let completed = store
            .persist_completed(&request, &response)
            .expect("persist completed response");
        assert_eq!(completed.id(), pending.id());
        assert_eq!(completed.certificate_hash(), pending.certificate_hash());
        assert_eq!(completed.response_hash(), HashOf::new(&response));
        assert_ne!(completed.payload_hash(), pending.payload_hash());
        assert_eq!(
            store
                .persist_completed(&request, &response)
                .expect("idempotent completion"),
            completed
        );

        drop(store);
        let (_store, recovery) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("reopen store");
        assert_eq!(recovery.len(), 1);
        assert_eq!(recovery.iter().len(), 1);
        let recovered = recovery.get(pending.id()).expect("recover exact request");
        assert_eq!(recovered.id(), pending.id());
        assert_eq!(recovered.request(), request.request());
        assert_eq!(recovered.certificate_hash(), pending.certificate_hash());
        let RecoveredCertifiedServePayloadState::Completed(completed_ref) = recovered.state()
        else {
            panic!("completed response metadata must recover");
        };
        assert_eq!(completed_ref.response_hash(), HashOf::new(&response));
        assert_eq!(completed_ref.manifest(), &response.manifest);
        assert_eq!(completed_ref.responder(), response.responder);
        assert_eq!(completed_ref.signature(), response.signature);
    }

    #[test]
    fn completed_payload_requires_exact_certified_responder_authority() {
        let temporary = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (request, response) = request_and_response(&context, &keys[0], 0, b"authslot".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let _pending_receipt = store.persist_pending(&request).expect("persist pending");

        let mut nonretaining = response.clone();
        nonretaining.responder = 3;
        nonretaining.signature =
            Signature::new(keys[3].private_key(), &nonretaining.signature_preimage())
                .payload()
                .to_vec();
        assert!(matches!(
            store.persist_completed(&request, &nonretaining),
            Err(CertifiedServePayloadStoreError::InvalidFrame { .. })
        ));

        let mut wrong_signer = response.clone();
        wrong_signer.signature =
            Signature::new(keys[1].private_key(), &wrong_signer.signature_preimage())
                .payload()
                .to_vec();
        assert!(matches!(
            store.persist_completed(&request, &wrong_signer),
            Err(CertifiedServePayloadStoreError::InvalidFrame { .. })
        ));
        let _completed_receipt = store
            .persist_completed(&request, &response)
            .expect("invalid attempts leave the pending request recoverable");
    }

    #[cfg(feature = "bls")]
    #[test]
    fn pending_receipt_requires_verified_qc_and_local_retention_authority() {
        let temporary = TempDir::new().expect("temporary directory");
        let (verified, keys) = verified_bls_context_and_keys();
        let forged = bls_request(&verified, &keys, false);
        let valid = bls_request(&verified, &keys, true);
        let outsider = KeyPair::try_from_seed(vec![0x99; 32], Algorithm::BlsNormal)
            .expect("deterministic non-roster BLS key");
        let mut foreign_context = verified.context().clone();
        foreign_context.leader_seed = [0xC9; 32];
        let foreign = VerifiedHeightContext::genesis(
            foreign_context,
            verified.proofs_of_possession().to_vec(),
        )
        .expect("verify foreign fixture height context");
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("open payload store");

        assert!(matches!(
            store.persist_pending_with_verified_retention(&foreign, &keys[0], &valid),
            Err(CertifiedServePayloadStoreError::ForeignVerifiedContext)
        ));
        assert!(matches!(
            store.persist_pending_with_verified_retention(&verified, &keys[0], &forged),
            Err(CertifiedServePayloadStoreError::InvalidAuthenticatedRequest(_))
        ));
        assert!(matches!(
            store.persist_pending_with_verified_retention(&verified, &keys[3], &valid),
            Err(CertifiedServePayloadStoreError::LocalRetentionAuthorityAbsent)
        ));
        assert!(matches!(
            store.persist_pending_with_verified_retention(&verified, &outsider, &valid),
            Err(CertifiedServePayloadStoreError::LocalRetentionAuthorityAbsent)
        ));
        assert!(store.indexed.is_empty());

        let receipt = store
            .persist_pending_with_verified_retention(&verified, &keys[0], &valid)
            .expect("verified certificate signer may retain the request");
        assert_eq!(receipt.local_retainer(), 0);
        assert_eq!(
            store
                .persist_pending_with_verified_retention(&verified, &keys[0], &valid)
                .expect("exact verified pending repeat is idempotent"),
            receipt
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovery_cut_reauthenticates_request_qc_and_typed_negative() {
        let temporary = TempDir::new().expect("temporary directory");
        let (verified, keys) = verified_bls_context_and_keys();
        let request = bls_request(&verified, &keys, true);
        let (mut store, empty_recovery) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("open payload store");
        let body_store = V2BodyStore::open(temporary.path(), verified.context().clone())
            .expect("open exact body store");
        assert!(
            empty_recovery
                .authenticate(&verified, &keys[0], &body_store)
                .expect("an empty observer cut requires no local retention authority")
                .is_empty()
        );
        let pending = store
            .persist_pending_with_verified_retention(&verified, &keys[0], &request)
            .expect("persist verified locally retained request");
        let outcome = CertifiedServePayloadNegativeOutcome::Rejected(19);
        let _negative_receipt = store
            .persist_negative(pending.id(), outcome)
            .expect("persist typed negative");
        drop(store);

        let (_store, recovery) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("reopen payload store");
        let authenticated = recovery
            .authenticate(&verified, &keys[0], &body_store)
            .expect("fully authenticate recovery cut");
        assert_eq!(authenticated.context_id(), verified.context().id());
        assert_eq!(authenticated.height(), verified.context().height);
        assert_eq!(authenticated.len(), 1);
        assert!(!authenticated.is_empty());
        assert_eq!(authenticated.iter().len(), 1);
        let recovered = authenticated
            .get(pending.id())
            .expect("resolve authenticated request");
        assert_eq!(recovered.id(), pending.id());
        assert_eq!(recovered.request().request_hash(), request.request_hash());
        assert_eq!(recovered.certificate_hash(), pending.certificate_hash());
        assert_ne!(recovered.payload_hash(), pending.payload_hash());
        assert!(matches!(
            recovered.state(),
            AuthenticatedRecoveredCertifiedServePayloadState::Negative(recovered)
                if *recovered == outcome
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovery_cut_derives_local_retention_from_the_actual_consensus_key() {
        let temporary = TempDir::new().expect("temporary directory");
        let (verified, keys) = verified_bls_context_and_keys();
        let request = bls_request(&verified, &keys, true);
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("open payload store");
        let _pending_receipt = store
            .persist_pending_with_verified_retention(&verified, &keys[0], &request)
            .expect("persist locally retained request");
        drop(store);
        let body_store = V2BodyStore::open(temporary.path(), verified.context().clone())
            .expect("open exact body store");
        let (_, recovery) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("reopen payload store");
        let outsider = KeyPair::try_from_seed(vec![0xE7; 32], Algorithm::BlsNormal)
            .expect("deterministic outsider key");

        assert!(matches!(
            recovery.authenticate(&verified, &outsider, &body_store),
            Err(CertifiedServePayloadRecoveryError::LocalRetentionAuthorityAbsent)
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn completed_payload_requires_exact_durable_body_receipt_and_bytes() {
        let temporary = TempDir::new().expect("temporary directory");
        let (verified, keys) = verified_bls_context_and_keys();
        let (body, manifest) = canonical_body_and_manifest(verified.context(), &keys, 0);
        let (other_body, other_manifest) =
            canonical_body_and_manifest(verified.context(), &keys, 1);
        let request =
            bls_request_for_subject(&verified, &keys, true, manifest.round, manifest.subject);
        let mut body_store = V2BodyStore::open(temporary.path(), verified.context().clone())
            .expect("open exact body store");
        let durable_body = body_store
            .store(manifest.clone(), body.clone())
            .expect("persist canonical response body");
        let other_durable_body = body_store
            .store(other_manifest, other_body)
            .expect("persist distinct canonical body");

        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("open payload store");
        let pending = payload_store
            .persist_pending_with_verified_retention(&verified, &keys[0], &request)
            .expect("persist verified locally retained request");
        let response =
            signed_certified_response(&request, manifest.clone(), body.clone(), 0, &keys);

        let mut foreign_context = verified.context().clone();
        foreign_context.leader_seed = [0xD3; 32];
        let foreign_body_store =
            V2BodyStore::open(temporary.path(), foreign_context).expect("open foreign body store");
        assert!(matches!(
            payload_store.persist_completed_with_durable_body(
                &request,
                &durable_body,
                &foreign_body_store,
                &response,
            ),
            Err(CertifiedServePayloadStoreError::ForeignBodyStore)
        ));
        assert!(matches!(
            payload_store.persist_completed_with_durable_body(
                &request,
                &other_durable_body,
                &body_store,
                &response,
            ),
            Err(CertifiedServePayloadStoreError::DurableBodyReceiptMismatch)
        ));

        let mut changed_manifest = manifest.clone();
        changed_manifest.chunk_root = Hash::new(b"changed response manifest root");
        let response_with_changed_manifest =
            signed_certified_response(&request, changed_manifest, body.clone(), 0, &keys);
        assert!(matches!(
            payload_store.persist_completed_with_durable_body(
                &request,
                &durable_body,
                &body_store,
                &response_with_changed_manifest,
            ),
            Err(CertifiedServePayloadStoreError::DurableBodyReceiptMismatch)
        ));

        let mut changed_body = body;
        changed_body[0] ^= 0x80;
        let response_with_changed_body =
            signed_certified_response(&request, manifest, changed_body, 0, &keys);
        assert!(matches!(
            payload_store.persist_completed_with_durable_body(
                &request,
                &durable_body,
                &body_store,
                &response_with_changed_body,
            ),
            Err(CertifiedServePayloadStoreError::DurableResponseBodyMismatch)
        ));

        let completed = payload_store
            .persist_completed_with_durable_body(&request, &durable_body, &body_store, &response)
            .expect("persist receipt-backed completed response");
        assert_eq!(completed.id(), pending.id());
        assert_eq!(completed.response_hash(), HashOf::new(&response));
        assert_eq!(
            payload_store
                .persist_completed_with_durable_body(
                    &request,
                    &durable_body,
                    &body_store,
                    &response,
                )
                .expect("exact receipt-backed completion is idempotent"),
            completed
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovery_cut_reconstructs_and_authenticates_completed_response() {
        let temporary = TempDir::new().expect("temporary directory");
        let (verified, keys) = verified_bls_context_and_keys();
        let (body, manifest) = canonical_body_and_manifest(verified.context(), &keys, 0);
        let request =
            bls_request_for_subject(&verified, &keys, true, manifest.round, manifest.subject);
        let mut body_store = V2BodyStore::open(temporary.path(), verified.context().clone())
            .expect("open exact body store");
        let durable_body = body_store
            .store(manifest.clone(), body.clone())
            .expect("persist canonical response body");

        let (mut payload_store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("open payload store");
        let pending = payload_store
            .persist_pending_with_verified_retention(&verified, &keys[0], &request)
            .expect("persist verified locally retained request");
        let responder = 1;
        let response = signed_certified_response(&request, manifest, body, responder, &keys);
        let _completed_receipt = payload_store
            .persist_completed_with_durable_body(&request, &durable_body, &body_store, &response)
            .expect("persist completed response");
        drop(payload_store);

        let (_payload_store, recovery) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("reopen payload store");
        let authenticated = recovery
            .authenticate(&verified, &keys[0], &body_store)
            .expect("local retainer authenticates an independently signed response");
        let recovered = authenticated
            .get(pending.id())
            .expect("resolve completed request");
        let AuthenticatedRecoveredCertifiedServePayloadState::Completed(completed) =
            recovered.state()
        else {
            panic!("completed response must remain terminal after authentication");
        };
        assert_eq!(completed.response(), &response);
        assert_eq!(HashOf::new(completed.response()), HashOf::new(&response));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn recovery_cut_rejects_a_structural_but_unauthenticated_qc() {
        let temporary = TempDir::new().expect("temporary directory");
        let (verified, keys) = verified_bls_context_and_keys();
        let request = bls_request(&verified, &keys, false);
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("open payload store");
        let _pending_receipt = store
            .persist_pending(&request)
            .expect("persist structurally valid request");
        drop(store);

        let body_store = V2BodyStore::open(temporary.path(), verified.context().clone())
            .expect("open exact body store");
        let (_store, recovery) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("reopen payload store");
        assert!(matches!(
            recovery.authenticate(&verified, &keys[0], &body_store),
            Err(CertifiedServePayloadRecoveryError::InvalidRequest(_))
        ));
    }

    #[test]
    fn negative_terminal_is_idempotent_and_cannot_be_replaced() {
        let temporary = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (request, response) = request_and_response(&context, &keys[0], 0, b"negative".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let pending = store.persist_pending(&request).expect("persist pending");
        let outcome = CertifiedServePayloadNegativeOutcome::Rejected(7);
        let negative = store
            .persist_negative(pending.id(), outcome)
            .expect("persist negative result");
        assert_eq!(negative.id(), pending.id());
        assert_eq!(negative.certificate_hash(), pending.certificate_hash());
        assert_eq!(negative.outcome(), outcome);
        assert_ne!(negative.payload_hash(), pending.payload_hash());
        assert_eq!(
            store
                .persist_negative(pending.id(), outcome)
                .expect("idempotent negative result"),
            negative
        );
        assert!(matches!(
            store.persist_negative(
                pending.id(),
                CertifiedServePayloadNegativeOutcome::Failed(7),
            ),
            Err(CertifiedServePayloadStoreError::TerminalConflict)
        ));
        assert!(matches!(
            store.persist_completed(&request, &response),
            Err(CertifiedServePayloadStoreError::TerminalConflict)
        ));
        assert!(matches!(
            store.persist_pending(&request),
            Err(CertifiedServePayloadStoreError::TerminalResurrection)
        ));

        drop(store);
        let (_store, recovery) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("reopen store");
        assert!(matches!(
            recovery
                .get(pending.id())
                .expect("recover negative request")
                .state(),
            RecoveredCertifiedServePayloadState::Negative(recovered) if recovered == outcome
        ));
    }

    #[test]
    fn capacity_is_checked_before_a_second_file_is_published() {
        let temporary = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (first, _) = request_and_response(&context, &keys[0], 0, b"first!!!".to_vec());
        let (second, _) = request_and_response(&context, &keys[0], 1, b"second!!".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open_with_max_entries(temporary.path(), &context, 1)
                .expect("open one-entry store");
        let _first_receipt = store
            .persist_pending(&first)
            .expect("persist first request");
        assert!(matches!(
            store.persist_pending(&second),
            Err(CertifiedServePayloadStoreError::PayloadCapacityExceeded { capacity: 1 })
        ));
        assert!(
            !store
                .path_for(CertifiedServePayloadId(second.request_hash()))
                .exists()
        );
    }

    #[test]
    fn first_directory_creation_fails_closed_until_its_parent_syncs() {
        let temporary = TempDir::new().expect("temporary directory");
        let root = temporary.path().join("fresh-root");
        let directory = root.join(STORE_DIRECTORY);
        let mut injected = false;
        let result = ensure_durable_directory_with(&directory, &mut |path| {
            if !injected && directory.exists() && path == root {
                injected = true;
                return Err(io_error(
                    "injected parent synchronisation failure",
                    path,
                    std::io::Error::other("injected failure"),
                ));
            }
            sync_directory(path)
        });
        assert!(matches!(
            result,
            Err(CertifiedServePayloadStoreError::Io { .. })
        ));
        assert!(injected);

        let (context, _) = context_and_keys();
        let (_store, recovery) = CertifiedServePayloadStoreV1::open(&root, &context)
            .expect("retry synchronises the existing directory before exposure");
        assert!(recovery.is_empty());
    }

    #[test]
    fn reopen_discards_regular_interrupted_file_but_rejects_corruption() {
        let temporary = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"recover!".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let pending = store.persist_pending(&request).expect("persist pending");
        let interrupted = store.temporary_path(pending.id());
        fs::write(&interrupted, b"interrupted frame").expect("write interrupted fixture");
        let final_path = store.path_for(pending.id());
        drop(store);

        let (store, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), &context)
            .expect("discard interrupted file");
        assert!(!interrupted.exists());
        assert_eq!(recovery.len(), 1);
        let mut frame = fs::read(&final_path).expect("read final frame");
        let last = frame.last_mut().expect("nonempty frame");
        *last ^= 0xFF;
        fs::write(&final_path, frame).expect("write corrupt frame fixture");
        drop(store);
        assert!(matches!(
            CertifiedServePayloadStoreV1::open(temporary.path(), &context),
            Err(CertifiedServePayloadStoreError::InvalidFrame { .. })
        ));
    }

    #[test]
    fn foreign_context_and_unexpected_entries_fail_closed() {
        let temporary = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"foreign!".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let _pending_receipt = store.persist_pending(&request).expect("persist pending");
        drop(store);

        let mut foreign = context.clone();
        foreign.leader_seed = [0xB8; 32];
        assert!(matches!(
            CertifiedServePayloadStoreV1::open(temporary.path(), &foreign),
            Err(CertifiedServePayloadStoreError::ForeignContext(_))
        ));

        fs::write(
            temporary.path().join(STORE_DIRECTORY).join("unexpected"),
            b"unexpected",
        )
        .expect("write unexpected fixture");
        assert!(matches!(
            CertifiedServePayloadStoreV1::open(temporary.path(), &context),
            Err(CertifiedServePayloadStoreError::UnexpectedEntry(_))
        ));
    }
}
