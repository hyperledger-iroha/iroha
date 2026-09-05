//! Crash-safe, scheduler-free payload storage for Certified-Serve lifecycles.
//!
//! One immutable canonical Pending file is owned by the hash of one exact
//! signed [`wire::CertifiedBodyRequest`]. A terminal result is an authenticated
//! append-only companion to that Pending frame. The full request is retained so
//! startup can independently reauthenticate it. Completed companions retain
//! only response metadata: canonical body bytes remain owned by the v2 body
//! store and must be resolved there before a response is reconstructed.
#[cfg(any(not(test), feature = "bls"))]
use super::v2_body_store::{DurableCertifiedServeBodyReadbackV1, V2BodyStoreInstanceIdentity};
use super::{
    v2::VerifiedHeightContext,
    v2_body_store::{DurableBodyReceipt, V2BodyStore},
    v2_transport::{
        AuthenticatedCertifiedBodyRequest, authenticate_certified_body_request_identity,
        authenticate_certified_body_request_with_verified_height,
    },
};
use iroha_crypto::{Hash, HashOf, KeyPair, Signature};
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::{Decode, DecodeAll as _, Encode};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{Read, Write},
    mem::size_of,
    path::{Path, PathBuf},
    sync::Arc,
};
use thiserror::Error;
const STORE_DIRECTORY: &str = "certified-serve-payload-v1";
const FILE_SUFFIX: &str = ".norito";
const TEMPORARY_FILE_SUFFIX: &str = ".norito.tmp";
const TERMINAL_FILE_SUFFIX: &str = ".norito.terminal";
const REMOVAL_FILE_SUFFIX: &str = ".norito.removed";
const QUARANTINE_FILE_SUFFIX: &str = ".norito.quarantine";
const MAX_QUARANTINED_STAGES_PER_HEIGHT: usize = 16;
const MAX_IN_FLIGHT_STAGES_PER_HEIGHT: usize = 1;
const FRAME_MAGIC: &[u8; 8] = b"SUM2SRV1";
const FORMAT_VERSION: u16 = 1;
const CHECKSUM_BYTES: usize = Hash::LENGTH;
const ADMISSION_RECEIPT_BINDING_DOMAIN: &[u8] =
    b"iroha:sumeragi:v2:certified-serve-admission-receipt:v1";
const FRAME_HEADER_BYTES: usize =
    FRAME_MAGIC.len() + size_of::<u16>() + size_of::<u64>() + CHECKSUM_BYTES;
const ENTRY_FIXED_HEADROOM_BYTES: u64 = 64 * 1024;
#[cfg(all(unix, not(target_os = "espidf")))]
const PRIVATE_LEAF_MODE: u32 = 0o600;
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
    coordinate_binding: Hash,
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
    /// Recheck the authenticated request and the complete receipt-coordinate
    /// binding without assuming a Pending or terminal frame shape.
    pub(super) fn exactly_matches_authenticated_coordinates(
        self,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> bool {
        self.id.request_hash() == authenticated.request_hash()
            && self.certificate_hash == HashOf::new(&authenticated.request().certificate)
            && self.coordinate_binding
                == admission_receipt_coordinate_binding(
                    self.id,
                    self.certificate_hash,
                    self.payload_hash,
                    self.local_retainer,
                )
    }
    /// Recompute the canonical Pending frame and compare every receipt
    /// coordinate with one exact authenticated request.
    pub(crate) fn exactly_matches_pending(
        self,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> bool {
        let request = authenticated.request();
        let payload = PersistedCertifiedServePayloadV1 {
            format_version: FORMAT_VERSION,
            context_id: request.round.context_id,
            height: request.round.height,
            request_hash: authenticated.request_hash(),
            request: request.clone(),
            state: PersistedCertifiedServePayloadStateV1::Pending,
        };
        self.id == payload.id()
            && self.certificate_hash == HashOf::new(&request.certificate)
            && self.payload_hash == payload.payload_hash()
            && self.coordinate_binding
                == admission_receipt_coordinate_binding(
                    self.id,
                    self.certificate_hash,
                    self.payload_hash,
                    self.local_retainer,
                )
    }
    /// Replace the signed-request identity in a negative fixture.
    #[cfg(test)]
    pub(crate) fn with_request_hash_for_test(
        mut self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
    ) -> Self {
        self.id = CertifiedServePayloadId(request_hash);
        self
    }
    /// Replace the certificate identity in a negative fixture.
    #[cfg(test)]
    pub(crate) fn with_certificate_hash_for_test(
        mut self,
        certificate_hash: HashOf<wire::QuorumCertificate>,
    ) -> Self {
        self.certificate_hash = certificate_hash;
        self
    }
    /// Replace the durable frame identity in a negative fixture.
    #[cfg(test)]
    pub(crate) fn with_payload_hash_for_test(mut self, payload_hash: Hash) -> Self {
        self.payload_hash = payload_hash;
        self
    }
    /// Replace the independently verified retainer in a negative fixture.
    #[cfg(test)]
    pub(crate) fn with_local_retainer_for_test(
        mut self,
        local_retainer: wire::ValidatorIndex,
    ) -> Self {
        self.local_retainer = local_retainer;
        self
    }
}
/// Sealed result of retaining one exact request for lifecycle admission.
///
/// A pending publication may be compensated after a conclusive rejection. A
/// terminal publication is valid only when the coordinator already owns the
/// matching durable row and can replay its tombstone.
#[derive(Debug)]
pub(super) struct DurableCertifiedServeAdmissionPublication {
    receipt: DurableCertifiedServeAdmissionReceipt,
    state: DurableCertifiedServeAdmissionStateV1,
    fresh_pending: bool,
}
impl Drop for DurableCertifiedServeAdmissionPublication {
    fn drop(&mut self) {}
}
/// Store-derived state needed to distinguish fresh admission from exact
/// tombstone replay without reopening or reconstructing payload bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(variant_size_differences)] // Completed retains its exact hash inline without allocation.
pub(super) enum DurableCertifiedServeAdmissionStateV1 {
    /// The exact request still owns a Pending frame.
    Pending,
    /// The exact request already owns this completed-response tombstone.
    Completed(HashOf<wire::CertifiedBodyResponse>),
    /// The exact request already owns this typed negative tombstone.
    Negative(CertifiedServePayloadNegativeOutcome),
}
impl DurableCertifiedServeAdmissionPublication {
    /// Exact durable request material used to project the lifecycle candidate.
    pub(super) const fn receipt(&self) -> DurableCertifiedServeAdmissionReceipt {
        self.receipt
    }
    /// Whether this publication may be removed as an unadmitted Pending frame.
    pub(super) const fn is_pending(&self) -> bool {
        matches!(self.state, DurableCertifiedServeAdmissionStateV1::Pending)
    }
    /// Whether this exact call created the Pending frame and therefore owns
    /// the sole authenticated pre-ledger abort right.
    pub(super) const fn can_abort_fresh_pending(&self) -> bool {
        self.is_pending() && self.fresh_pending
    }
    /// Return the exact state read or created by this store transaction.
    pub(super) const fn state(&self) -> DurableCertifiedServeAdmissionStateV1 {
        self.state
    }
    /// Recheck the request, certificate, and receipt coordinates sealed by the
    /// store before a terminal lifecycle row may stutter or replay.
    pub(super) fn exactly_matches_authenticated_request(
        &self,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> bool {
        self.receipt
            .exactly_matches_authenticated_coordinates(authenticated)
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
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RecoveredCertifiedServeCompletedPayload<'a> {
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    manifest: &'a wire::PayloadManifest,
    responder: wire::ValidatorIndex,
    signature: &'a [u8],
}
#[cfg(test)]
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
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(
    variant_size_differences,
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
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
#[must_use]
pub(crate) struct RecoveredCertifiedServePayload<'a> {
    payload: &'a PersistedCertifiedServePayloadV1,
}
#[cfg(test)]
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
                    signature: signature.as_slice(),
                },
            ),
            PersistedCertifiedServePayloadStateV1::Negative { outcome } => {
                RecoveredCertifiedServePayloadState::Negative(*outcome)
            }
        }
    }
}
/// Fully authenticated terminal response metadata.
///
/// Ordinary startup additionally reloads and hashes the canonical body. The
/// CompleteTip retirement-only path verifies the same request/manifest/body-
/// hash/certified-retainer responder signature but deliberately retains no
/// executable body.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub(crate) struct AuthenticatedRecoveredCertifiedServeResponse {
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    manifest: wire::PayloadManifest,
    responder: wire::ValidatorIndex,
    signature: Vec<u8>,
    body_revalidated: bool,
}
impl AuthenticatedRecoveredCertifiedServeResponse {
    /// Hash of the exact response whose signed metadata was authenticated.
    pub(crate) const fn response_hash(&self) -> HashOf<wire::CertifiedBodyResponse> {
        self.response_hash
    }
    /// Whether this authentication independently reloaded the canonical body.
    ///
    /// Retirement-only metadata authentication deliberately returns false. A
    /// caller may compare that metadata with an already-terminal exact ledger
    /// family, but must not use it to promote a Pending ledger row.
    pub(crate) const fn permits_payload_store_ahead_terminal_rebind(&self) -> bool {
        self.body_revalidated
    }
}
/// Closed post-authentication state of one recovered Serve payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AuthenticatedRecoveredCertifiedServePayloadState {
    /// The exact request remains pending physical execution.
    Pending,
    /// Complete signed response metadata was reauthenticated; ordinary startup
    /// also reconstructed the independently durable body bytes.
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
    local_retainer: wire::ValidatorIndex,
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
    /// Independently verified frozen-roster index retaining this request.
    pub(crate) const fn local_retainer(&self) -> wire::ValidatorIndex {
        self.local_retainer
    }
    /// Borrow the exact post-authentication recovery state.
    pub(crate) const fn state(&self) -> &AuthenticatedRecoveredCertifiedServePayloadState {
        &self.state
    }
    /// Recompute the exact canonical payload frame from the authenticated
    /// request and state retained by this recovery cut.
    pub(crate) fn exactly_matches_persisted_payload(&self) -> bool {
        let state = match &self.state {
            AuthenticatedRecoveredCertifiedServePayloadState::Pending => {
                PersistedCertifiedServePayloadStateV1::Pending
            }
            AuthenticatedRecoveredCertifiedServePayloadState::Completed(completed) => {
                PersistedCertifiedServePayloadStateV1::Completed {
                    response_hash: completed.response_hash,
                    manifest: completed.manifest.clone(),
                    responder: completed.responder,
                    signature: completed.signature.clone(),
                }
            }
            AuthenticatedRecoveredCertifiedServePayloadState::Negative(outcome) => {
                PersistedCertifiedServePayloadStateV1::Negative { outcome: *outcome }
            }
        };
        let request = self.request.request();
        PersistedCertifiedServePayloadV1 {
            format_version: FORMAT_VERSION,
            context_id: request.round.context_id,
            height: request.round.height,
            request_hash: self.request.request_hash(),
            request: request.clone(),
            state,
        }
        .payload_hash()
            == self.payload_hash
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
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.payloads.len()
    }
    /// Whether no payload survived authenticated recovery.
    #[cfg(test)]
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
    fn retain_owned(&mut self, retained: &BTreeSet<CertifiedServePayloadId>) {
        self.payloads.retain(|id, _| retained.contains(id));
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
    #[cfg(test)]
    pub(crate) const fn context_id(&self) -> wire::HeightContextId {
        self.context_id
    }
    /// Exact consensus height owning this cut.
    #[cfg(test)]
    pub(crate) const fn height(&self) -> wire::Height {
        self.height
    }
    /// Number of recovered exact requests.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.payloads.len()
    }
    /// Whether no Certified-Serve payload was recovered.
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.payloads.is_empty()
    }
    /// Resolve one recovered request by its exact signed-request hash.
    #[cfg(test)]
    pub(crate) fn get(
        &self,
        id: CertifiedServePayloadId,
    ) -> Option<RecoveredCertifiedServePayload<'_>> {
        self.payloads
            .get(&id)
            .map(|payload| RecoveredCertifiedServePayload { payload })
    }
    /// Iterate in canonical request-hash order.
    #[cfg(test)]
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
        self.authenticate_inner(verified, local_signer, Some(body_store))
    }
    /// Authenticate the retirement-only payload census without reopening body bytes.
    ///
    /// Completed metadata is still bound to the exact request, manifest body
    /// hash, certified-retainer responder signature, canonical payload frame,
    /// and durable response hash. This cut is suitable only for terminal
    /// ledger reconciliation; it cannot reconstruct or serve a response body.
    pub(in crate::sumeragi) fn authenticate_for_complete_tip_retirement(
        self,
        verified: &VerifiedHeightContext,
        local_signer: &KeyPair,
    ) -> Result<AuthenticatedCertifiedServePayloadRecoveryCut, CertifiedServePayloadRecoveryError>
    {
        self.authenticate_inner(verified, local_signer, None)
    }
    fn authenticate_inner(
        self,
        verified: &VerifiedHeightContext,
        local_signer: &KeyPair,
        body_store: Option<&V2BodyStore>,
    ) -> Result<AuthenticatedCertifiedServePayloadRecoveryCut, CertifiedServePayloadRecoveryError>
    {
        let context = verified.context();
        if self.context_id != context.id() || self.height != context.height {
            return Err(CertifiedServePayloadRecoveryError::ForeignContext);
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
                    if request
                        .request()
                        .certificate
                        .signers
                        .binary_search(&persisted_responder)
                        .is_err()
                    {
                        return Err(CertifiedServePayloadRecoveryError::InvalidResponse(
                            "persisted response signer lost certified local retention authority"
                                .to_owned(),
                        ));
                    }
                    let responder_peer = &context.roster[responder_index].validator;
                    manifest.validate(context).map_err(|error| {
                        CertifiedServePayloadRecoveryError::InvalidResponse(error.to_string())
                    })?;
                    if manifest.round != request.request().round
                        || manifest.subject != request.request().subject
                        || signature.is_empty()
                        || signature.len() > wire::MAX_CONSENSUS_SIGNATURE_BYTES
                    {
                        return Err(CertifiedServePayloadRecoveryError::InvalidResponse(
                            "persisted response metadata changed its request binding".to_owned(),
                        ));
                    }
                    let signed_payload = wire::CertifiedBodyResponseSignaturePayload {
                        protocol_version: wire::PROTOCOL_VERSION,
                        request_hash: request.request_hash(),
                        manifest: manifest.clone(),
                        body_hash: manifest.subject.payload_hash,
                        responder: responder_peer.clone(),
                    };
                    let mut preimage = b"iroha:sumeragi:v2:certified-body-response".to_vec();
                    preimage.extend_from_slice(&signed_payload.encode());
                    let response_signature =
                        Signature::try_from_bytes(&signature).map_err(|error| {
                            CertifiedServePayloadRecoveryError::InvalidResponse(error.to_string())
                        })?;
                    response_signature
                        .verify(responder_peer.public_key(), &preimage)
                        .map_err(|error| {
                            CertifiedServePayloadRecoveryError::InvalidResponse(error.to_string())
                        })?;
                    if let Some(body_store) = body_store {
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
                            manifest: manifest.clone(),
                            body,
                            responder: responder_peer.clone(),
                            signature: signature.clone(),
                        };
                        response
                            .validate_against(context, request.request(), responder_peer)
                            .map_err(|error| {
                                CertifiedServePayloadRecoveryError::InvalidResponse(
                                    error.to_string(),
                                )
                            })?;
                        if HashOf::new(&response) != response_hash {
                            return Err(CertifiedServePayloadRecoveryError::ResponseHashMismatch);
                        }
                    }
                    AuthenticatedRecoveredCertifiedServePayloadState::Completed(
                        AuthenticatedRecoveredCertifiedServeResponse {
                            response_hash,
                            manifest,
                            responder: persisted_responder,
                            signature,
                            body_revalidated: body_store.is_some(),
                        },
                    )
                }
            };
            let recovered = AuthenticatedRecoveredCertifiedServePayload {
                request,
                payload_hash,
                local_retainer,
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

/// Failure while refreshing the exact live Serve census for height retirement.
#[derive(Debug, Error)]
pub(in crate::sumeragi) enum CertifiedServeRetirementAuthenticationErrorV1 {
    /// The caller did not retain the exact launched service/store authority.
    #[error("Certified-Serve retirement used a foreign lifecycle service owner")]
    ForeignServiceOwner,
    /// The live coordinator, ledger, admission waits, and payload cut drifted.
    #[error("Certified-Serve retirement census is not an exact live lifecycle cut")]
    InvalidLifecycleCensus,
    /// The retained open store changed behind its sole process owner.
    #[error(transparent)]
    Store(#[from] CertifiedServePayloadStoreError),
    /// Current durable payloads failed verified retirement authentication.
    #[error(transparent)]
    Recovery(#[from] CertifiedServePayloadRecoveryError),
}

/// Failure while opening or advancing the Certified-Serve payload store.
#[derive(Debug, Error)]
#[allow(variant_size_differences)]
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
    /// Emergency Fast startup owns an inert payload store and cannot mutate it.
    #[error("Certified-Serve payload store is read-only during emergency Fast startup")]
    EmergencyFastReadOnly,
    /// The store directory contains a name outside the closed format.
    #[error("unexpected Certified-Serve payload entry: {}", .0.display())]
    UnexpectedEntry(PathBuf),
    /// A directory entry is a symlink or another non-regular file.
    #[error("Certified-Serve payload entry is not a regular file: {}", .0.display())]
    NonRegularEntry(PathBuf),
    /// The retained store directory was replaced by a symlink or non-directory.
    #[error("Certified-Serve payload store target is not the retained directory: {}", .0.display())]
    InvalidStoreDirectory(PathBuf),
    /// A Kura-minted directory authority no longer names the exact production target.
    #[error("invalid Certified-Serve payload storage binding at {}: {reason}", path.display())]
    StorageBinding {
        /// Expected Kura-derived target.
        path: PathBuf,
        /// Stable rejection reason.
        reason: &'static str,
    },
    /// This platform cannot provide the descriptor-relative storage contract.
    #[error("descriptor-relative Certified-Serve payload storage is unsupported at {}", .0.display())]
    UnsupportedStorageBinding(PathBuf),
    /// A canonical destination did not match the expected absence or exact incumbent frame.
    #[error("Certified-Serve payload destination changed or already exists: {}", .0.display())]
    PublicationConflict(PathBuf),
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
    #[cfg(test)]
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
/// Admission-only classification of a failed payload retention attempt.
///
/// `PublicationAmbiguous` means exclusive staging began and startup recovery
/// must decide whether to quarantine, resume, or accept the final frame. The
/// caller must retain all ownership and restart before retrying.
#[derive(Debug)]
pub(super) enum CertifiedServePayloadRetentionError {
    /// The canonical final path was not changed by this attempt.
    Unchanged(CertifiedServePayloadStoreError),
    /// Exclusive staging began, so durable publication requires recovery.
    PublicationAmbiguous(CertifiedServePayloadStoreError),
}
/// Terminal-write classification preserving whether a terminal stage or
/// companion may already be durable.
#[derive(Debug, Error)]
pub(super) enum CertifiedServeTerminalPersistenceError {
    /// Caller-supplied request/body/response material was rejected before the
    /// retained payload frame was opened or changed.
    #[error("Certified-Serve terminal input was rejected: {0}")]
    InputRejected(CertifiedServePayloadStoreError),
    /// The retained payload/body-store authority was missing, corrupt, or in
    /// conflict before this attempt changed the final payload path.
    #[error("Certified-Serve terminal store invariant failed: {0}")]
    StoreInvariant(CertifiedServePayloadStoreError),
    /// Exclusive staging began, so durable publication requires recovery.
    #[error("Certified-Serve terminal publication is durability-ambiguous: {0}")]
    PublicationAmbiguous(CertifiedServePayloadStoreError),
}
impl CertifiedServeTerminalPersistenceError {
    #[cfg(test)]
    fn into_store_error(self) -> CertifiedServePayloadStoreError {
        match self {
            Self::InputRejected(error)
            | Self::StoreInvariant(error)
            | Self::PublicationAmbiguous(error) => error,
        }
    }
}
impl CertifiedServePayloadRetentionError {
    #[cfg(test)]
    fn into_store_error(self) -> CertifiedServePayloadStoreError {
        match self {
            Self::Unchanged(error) | Self::PublicationAmbiguous(error) => error,
        }
    }
}
enum PersistPayloadError {
    Unpublished(CertifiedServePayloadStoreError),
    PublishedButUnsynchronized(CertifiedServePayloadStoreError),
}
impl PersistPayloadError {
    fn into_store_error(self) -> CertifiedServePayloadStoreError {
        match self {
            Self::Unpublished(error) | Self::PublishedButUnsynchronized(error) => error,
        }
    }
    fn into_retention_error(self) -> CertifiedServePayloadRetentionError {
        match self {
            Self::Unpublished(error) => CertifiedServePayloadRetentionError::Unchanged(error),
            Self::PublishedButUnsynchronized(error) => {
                CertifiedServePayloadRetentionError::PublicationAmbiguous(error)
            }
        }
    }
    fn into_terminal_error(self) -> CertifiedServeTerminalPersistenceError {
        match self {
            Self::Unpublished(error) => {
                CertifiedServeTerminalPersistenceError::StoreInvariant(error)
            }
            Self::PublishedButUnsynchronized(error) => {
                CertifiedServeTerminalPersistenceError::PublicationAmbiguous(error)
            }
        }
    }
}

#[cfg(all(unix, not(target_os = "espidf")))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CertifiedServeStorageIdentity {
    device: u64,
    inode: u64,
}

#[cfg(all(unix, not(target_os = "espidf")))]
impl CertifiedServeStorageIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt as _;

        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }

    fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev as u64,
            inode: stat.st_ino as u64,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BoundCertifiedServePayloadLeaf {
    name: OsString,
    length: u64,
    frame_hash: Option<Hash>,
    #[cfg(all(unix, not(target_os = "espidf")))]
    identity: CertifiedServeStorageIdentity,
}

/// Descriptor-relative exclusive owner of the exact payload directory.
#[derive(Debug)]
struct BoundCertifiedServePayloadDirectory {
    expected_path: PathBuf,
    #[cfg(all(unix, not(target_os = "espidf")))]
    canonical_path: PathBuf,
    #[cfg(all(unix, not(target_os = "espidf")))]
    directory: File,
    #[cfg(all(unix, not(target_os = "espidf")))]
    identity: CertifiedServeStorageIdentity,
}

#[cfg(target_vendor = "apple")]
#[allow(unsafe_code)]
fn has_apple_extended_acl(
    file: &File,
    path: &Path,
) -> Result<bool, CertifiedServePayloadStoreError> {
    use std::{
        ffi::{c_int, c_void},
        os::fd::AsRawFd as _,
    };

    const ACL_TYPE_EXTENDED: c_int = 0x0000_0100;
    const ACL_FIRST_ENTRY: c_int = 0;
    const ENOENT: i32 = 2;
    const EINVAL: i32 = 22;

    unsafe extern "C" {
        fn acl_get_fd_np(fd: c_int, acl_type: c_int) -> *mut c_void;
        fn acl_get_entry(acl: *mut c_void, entry_id: c_int, entry: *mut *mut c_void) -> c_int;
        fn acl_free(value: *mut c_void) -> c_int;
    }

    struct OwnedAcl(*mut c_void);
    impl Drop for OwnedAcl {
        fn drop(&mut self) {
            // SAFETY: this non-null pointer was returned by `acl_get_fd_np`
            // and this owner releases it exactly once.
            let _ = unsafe { acl_free(self.0) };
        }
    }

    // SAFETY: `file` retains a live descriptor and the ACL type is the Apple
    // extended ACL type declared by `<sys/acl.h>` on every supported Apple SDK.
    let raw_acl = unsafe { acl_get_fd_np(file.as_raw_fd(), ACL_TYPE_EXTENDED) };
    if raw_acl.is_null() {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ENOENT) {
            return Ok(false);
        }
        return Err(io_error(
            "inspect descriptor-bound Apple extended ACL",
            path,
            error,
        ));
    }
    let acl = OwnedAcl(raw_acl);
    let mut entry = std::ptr::null_mut();
    // SAFETY: `acl` owns a live ACL object and `entry` is writable output.
    if unsafe { acl_get_entry(acl.0, ACL_FIRST_ENTRY, &mut entry) } == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(EINVAL) {
        return Ok(false);
    }
    Err(io_error(
        "enumerate descriptor-bound Apple extended ACL",
        path,
        error,
    ))
}

#[cfg(not(target_vendor = "apple"))]
fn has_apple_extended_acl(
    _file: &File,
    _path: &Path,
) -> Result<bool, CertifiedServePayloadStoreError> {
    Ok(false)
}

impl BoundCertifiedServePayloadDirectory {
    #[cfg(test)]
    fn open_or_create(path: &Path) -> Result<Self, CertifiedServePayloadStoreError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            ensure_durable_directory(path)?;
            let canonical_path = fs::canonicalize(path)
                .map_err(|source| io_error("canonicalize store directory", path, source))?;
            let directory = File::from(
                rustix::fs::open(
                    path,
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                )
                .map_err(std::io::Error::from)
                .map_err(|source| io_error("open store directory", path, source))?,
            );
            Self::from_opened_directory(path.to_path_buf(), canonical_path, directory)
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            Err(CertifiedServePayloadStoreError::UnsupportedStorageBinding(
                path.to_path_buf(),
            ))
        }
    }

    fn from_kura_authority(
        kura: &crate::kura::Kura,
        authority: crate::kura::KuraV2CertifiedServePayloadDirectoryAuthority,
        context: &wire::HeightContext,
    ) -> Result<Self, CertifiedServePayloadStoreError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            let expected_path = kura
                .sumeragi_v2_storage_root()
                .join("lifecycle-v1")
                .join(hex::encode(context.id().0.as_ref()))
                .join(STORE_DIRECTORY);
            if !authority.matches_kura(kura) {
                return Err(CertifiedServePayloadStoreError::StorageBinding {
                    path: expected_path,
                    reason: "payload-store authority belongs to another Kura instance",
                });
            }
            if !authority.matches_context(context) {
                return Err(CertifiedServePayloadStoreError::StorageBinding {
                    path: expected_path,
                    reason: "payload-store authority belongs to another height context",
                });
            }
            let (authority_path, mint_time_canonical_path, directory) = authority
                .into_opened_directory_for(kura, context)
                .ok_or_else(|| CertifiedServePayloadStoreError::StorageBinding {
                    path: expected_path.clone(),
                    reason: "payload-store authority changed after Kura minted it",
                })?;
            if authority_path != expected_path {
                return Err(CertifiedServePayloadStoreError::StorageBinding {
                    path: authority_path,
                    reason: "payload-store authority names a non-canonical Kura path",
                });
            }
            return Self::from_opened_directory(expected_path, mint_time_canonical_path, directory);
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = (authority, context);
            Err(CertifiedServePayloadStoreError::UnsupportedStorageBinding(
                kura.sumeragi_v2_storage_root().join("lifecycle-v1"),
            ))
        }
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn from_opened_directory(
        expected_path: PathBuf,
        mint_time_canonical_path: PathBuf,
        directory: File,
    ) -> Result<Self, CertifiedServePayloadStoreError> {
        use std::os::unix::fs::MetadataExt as _;

        let lexical = fs::symlink_metadata(&expected_path)
            .map_err(|source| io_error("inspect store directory", &expected_path, source))?;
        let canonical_path = fs::canonicalize(&expected_path)
            .map_err(|source| io_error("canonicalize store directory", &expected_path, source))?;
        let opened = directory
            .metadata()
            .map_err(|source| io_error("inspect opened store directory", &expected_path, source))?;
        let identity = CertifiedServeStorageIdentity::from_metadata(&opened);
        if expected_path.file_name() != Some(OsStr::new(STORE_DIRECTORY))
            || canonical_path != mint_time_canonical_path
            || lexical.file_type().is_symlink()
            || !lexical.is_dir()
            || !opened.is_dir()
            || lexical.uid() != rustix::process::geteuid().as_raw()
            || opened.uid() != rustix::process::geteuid().as_raw()
            || lexical.mode() & 0o022 != 0
            || opened.mode() & 0o022 != 0
            || CertifiedServeStorageIdentity::from_metadata(&lexical) != identity
        {
            return Err(CertifiedServePayloadStoreError::InvalidStoreDirectory(
                expected_path,
            ));
        }
        if has_apple_extended_acl(&directory, &expected_path)? {
            return Err(CertifiedServePayloadStoreError::InvalidStoreDirectory(
                expected_path,
            ));
        }
        rustix::fs::flock(
            &directory,
            rustix::fs::FlockOperation::NonBlockingLockExclusive,
        )
        .map_err(std::io::Error::from)
        .map_err(|source| io_error("lock store directory", &expected_path, source))?;
        let bound = Self {
            expected_path,
            canonical_path,
            directory,
            identity,
        };
        bound.verify_linked()?;
        Ok(bound)
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn verify_linked(&self) -> Result<(), CertifiedServePayloadStoreError> {
        use std::os::unix::fs::MetadataExt as _;

        let lexical = fs::symlink_metadata(&self.expected_path).map_err(|source| {
            io_error(
                "inspect linked store directory",
                &self.expected_path,
                source,
            )
        })?;
        let canonical = fs::canonicalize(&self.expected_path).map_err(|source| {
            io_error(
                "canonicalize linked store directory",
                &self.expected_path,
                source,
            )
        })?;
        let retained = self.directory.metadata().map_err(|source| {
            io_error(
                "inspect retained store directory",
                &self.expected_path,
                source,
            )
        })?;
        if lexical.file_type().is_symlink()
            || !lexical.is_dir()
            || !retained.is_dir()
            || lexical.uid() != rustix::process::geteuid().as_raw()
            || retained.uid() != rustix::process::geteuid().as_raw()
            || lexical.mode() & 0o022 != 0
            || retained.mode() & 0o022 != 0
            || canonical != self.canonical_path
            || CertifiedServeStorageIdentity::from_metadata(&lexical) != self.identity
            || CertifiedServeStorageIdentity::from_metadata(&retained) != self.identity
        {
            return Err(CertifiedServePayloadStoreError::InvalidStoreDirectory(
                self.expected_path.clone(),
            ));
        }
        if has_apple_extended_acl(&self.directory, &self.expected_path)? {
            return Err(CertifiedServePayloadStoreError::InvalidStoreDirectory(
                self.expected_path.clone(),
            ));
        }
        Ok(())
    }

    fn is_linked(&self) -> bool {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            self.verify_linked().is_ok()
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            false
        }
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn fresh_handle(&self) -> Result<File, CertifiedServePayloadStoreError> {
        self.verify_linked()?;
        let directory = File::from(
            rustix::fs::openat(
                &self.directory,
                ".",
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(std::io::Error::from)
            .map_err(|source| {
                io_error(
                    "reopen retained store directory",
                    &self.expected_path,
                    source,
                )
            })?,
        );
        let metadata = directory.metadata().map_err(|source| {
            io_error(
                "inspect reopened store directory",
                &self.expected_path,
                source,
            )
        })?;
        if !metadata.is_dir()
            || CertifiedServeStorageIdentity::from_metadata(&metadata) != self.identity
        {
            return Err(CertifiedServePayloadStoreError::InvalidStoreDirectory(
                self.expected_path.clone(),
            ));
        }
        self.verify_linked()?;
        Ok(directory)
    }

    fn inventory(
        &self,
        capacity: usize,
    ) -> Result<Vec<BoundCertifiedServePayloadLeaf>, CertifiedServePayloadStoreError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            use std::os::unix::ffi::OsStrExt as _;

            let directory = self.fresh_handle()?;
            let entries = rustix::fs::Dir::read_from(&directory)
                .map_err(std::io::Error::from)
                .map_err(|source| io_error("read store directory", &self.expected_path, source))?;
            let mut leaves = Vec::new();
            for entry in entries {
                let entry = entry.map_err(std::io::Error::from).map_err(|source| {
                    io_error("read store directory entry", &self.expected_path, source)
                })?;
                let name = OsStr::from_bytes(entry.file_name().to_bytes());
                if matches!(name.as_bytes(), b"." | b"..") {
                    continue;
                }
                if leaves.len() >= capacity {
                    return Err(CertifiedServePayloadStoreError::DirectoryCapacityExceeded {
                        capacity,
                    });
                }
                let name = name.to_os_string();
                let path = self.expected_path.join(&name);
                let stat =
                    rustix::fs::statat(&directory, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
                        .map_err(std::io::Error::from)
                        .map_err(|source| io_error("inspect store entry", &path, source))?;
                if rustix::fs::FileType::from_raw_mode(stat.st_mode)
                    != rustix::fs::FileType::RegularFile
                    || stat.st_nlink as u64 != 1
                    || stat.st_uid != rustix::process::geteuid().as_raw()
                    || stat.st_size < 0
                    || u32::from(stat.st_mode) & 0o7777 != PRIVATE_LEAF_MODE
                {
                    return Err(CertifiedServePayloadStoreError::NonRegularEntry(path));
                }
                leaves.push(BoundCertifiedServePayloadLeaf {
                    name,
                    length: u64::try_from(stat.st_size).map_err(|_| {
                        CertifiedServePayloadStoreError::NonRegularEntry(path.clone())
                    })?,
                    frame_hash: None,
                    identity: CertifiedServeStorageIdentity::from_stat(&stat),
                });
            }
            self.verify_linked()?;
            Ok(leaves)
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = capacity;
            Err(CertifiedServePayloadStoreError::UnsupportedStorageBinding(
                self.expected_path.clone(),
            ))
        }
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn inspect_leaf(
        &self,
        name: &OsStr,
        maximum: u64,
    ) -> Result<Option<BoundCertifiedServePayloadLeaf>, CertifiedServePayloadStoreError> {
        let path = self.expected_path.join(name);
        let stat = match rustix::fs::statat(
            &self.directory,
            name,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(stat) => stat,
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => {
                return Err(io_error(
                    "inspect store leaf",
                    &path,
                    std::io::Error::from(error),
                ));
            }
        };
        if rustix::fs::FileType::from_raw_mode(stat.st_mode) != rustix::fs::FileType::RegularFile
            || stat.st_nlink as u64 != 1
            || stat.st_uid != rustix::process::geteuid().as_raw()
            || stat.st_size < 0
            || u32::from(stat.st_mode) & 0o7777 != PRIVATE_LEAF_MODE
        {
            return Err(CertifiedServePayloadStoreError::NonRegularEntry(path));
        }
        let length = u64::try_from(stat.st_size)
            .map_err(|_| CertifiedServePayloadStoreError::NonRegularEntry(path.clone()))?;
        if length > maximum {
            return Err(CertifiedServePayloadStoreError::EntryTooLarge {
                actual: length,
                bound: maximum,
            });
        }
        Ok(Some(BoundCertifiedServePayloadLeaf {
            name: name.to_os_string(),
            length,
            frame_hash: None,
            identity: CertifiedServeStorageIdentity::from_stat(&stat),
        }))
    }

    #[cfg(not(all(unix, not(target_os = "espidf"))))]
    fn inspect_leaf(
        &self,
        _name: &OsStr,
        _maximum: u64,
    ) -> Result<Option<BoundCertifiedServePayloadLeaf>, CertifiedServePayloadStoreError> {
        Err(CertifiedServePayloadStoreError::UnsupportedStorageBinding(
            self.expected_path.clone(),
        ))
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn verify_open_leaf(
        &self,
        file: &File,
        expected: &BoundCertifiedServePayloadLeaf,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        use std::os::unix::fs::MetadataExt as _;

        let path = self.expected_path.join(&expected.name);
        let opened = file
            .metadata()
            .map_err(|source| io_error("inspect opened store leaf", &path, source))?;
        let linked = self
            .inspect_leaf(&expected.name, expected.length)?
            .ok_or_else(|| CertifiedServePayloadStoreError::NonRegularEntry(path.clone()))?;
        if !opened.is_file()
            || opened.nlink() != 1
            || opened.uid() != rustix::process::geteuid().as_raw()
            || opened.len() != expected.length
            || opened.mode() & 0o7777 != PRIVATE_LEAF_MODE
            || CertifiedServeStorageIdentity::from_metadata(&opened) != expected.identity
            || linked.identity != expected.identity
            || linked.length != expected.length
        {
            return Err(CertifiedServePayloadStoreError::NonRegularEntry(path));
        }
        if has_apple_extended_acl(file, &path)? {
            return Err(CertifiedServePayloadStoreError::NonRegularEntry(path));
        }
        Ok(())
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn open_leaf(
        &self,
        expected: &BoundCertifiedServePayloadLeaf,
    ) -> Result<File, CertifiedServePayloadStoreError> {
        let path = self.expected_path.join(&expected.name);
        let current = self
            .inspect_leaf(&expected.name, expected.length)?
            .ok_or_else(|| CertifiedServePayloadStoreError::NonRegularEntry(path.clone()))?;
        if current.identity != expected.identity || current.length != expected.length {
            return Err(CertifiedServePayloadStoreError::NonRegularEntry(path));
        }
        let file = File::from(
            rustix::fs::openat(
                &self.directory,
                &expected.name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NONBLOCK,
                rustix::fs::Mode::empty(),
            )
            .map_err(std::io::Error::from)
            .map_err(|source| io_error("open store leaf", &path, source))?,
        );
        self.verify_open_leaf(&file, expected)?;
        self.verify_linked()?;
        Ok(file)
    }

    fn read_leaf(
        &self,
        leaf: &BoundCertifiedServePayloadLeaf,
        maximum: u64,
    ) -> Result<Vec<u8>, CertifiedServePayloadStoreError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            if leaf.length > maximum {
                return Err(CertifiedServePayloadStoreError::EntryTooLarge {
                    actual: leaf.length,
                    bound: maximum,
                });
            }
            let path = self.expected_path.join(&leaf.name);
            let mut file = self.open_leaf(leaf)?;
            let mut bytes = Vec::with_capacity(usize::try_from(leaf.length).unwrap_or(0));
            Read::by_ref(&mut file)
                .take(maximum.saturating_add(1))
                .read_to_end(&mut bytes)
                .map_err(|source| io_error("read store leaf", &path, source))?;
            let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            if actual != leaf.length || actual > maximum {
                return Err(CertifiedServePayloadStoreError::EntryTooLarge {
                    actual,
                    bound: maximum,
                });
            }
            self.verify_open_leaf(&file, leaf)?;
            self.verify_linked()?;
            Ok(bytes)
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = (leaf, maximum);
            Err(CertifiedServePayloadStoreError::UnsupportedStorageBinding(
                self.expected_path.clone(),
            ))
        }
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn validate_publication_destination(
        &self,
        destination: &OsStr,
        maximum: u64,
        expected: Option<&BoundCertifiedServePayloadLeaf>,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        let destination_path = self.expected_path.join(destination);
        let observed = self.inspect_leaf(destination, maximum)?;
        match (expected, observed.as_ref()) {
            (None, None) => Ok(()),
            (Some(expected), Some(observed)) => {
                let Some(expected_frame_hash) = expected.frame_hash.as_ref() else {
                    return Err(CertifiedServePayloadStoreError::PublicationConflict(
                        destination_path,
                    ));
                };
                if expected.name.as_os_str() != destination
                    || expected.identity != observed.identity
                    || expected.length != observed.length
                    || !Hash::new(&self.read_leaf(observed, maximum)?).eq(expected_frame_hash)
                {
                    return Err(CertifiedServePayloadStoreError::PublicationConflict(
                        destination_path,
                    ));
                }
                Ok(())
            }
            (None, Some(_)) => Err(CertifiedServePayloadStoreError::PublicationConflict(
                destination_path,
            )),
            (Some(_), None) => Err(CertifiedServePayloadStoreError::UnknownPayload),
        }
    }

    #[cfg(not(all(unix, not(target_os = "espidf"))))]
    fn validate_publication_destination(
        &self,
        _destination: &OsStr,
        _maximum: u64,
        _expected: Option<&BoundCertifiedServePayloadLeaf>,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        Err(CertifiedServePayloadStoreError::UnsupportedStorageBinding(
            self.expected_path.clone(),
        ))
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn exact_leaf_at(
        &self,
        name: &OsStr,
        maximum: u64,
    ) -> Result<Option<BoundCertifiedServePayloadLeaf>, CertifiedServePayloadStoreError> {
        let Some(mut leaf) = self.inspect_leaf(name, maximum)? else {
            return Ok(None);
        };
        leaf.frame_hash = Some(Hash::new(self.read_leaf(&leaf, maximum)?));
        Ok(Some(leaf))
    }

    #[cfg(not(all(unix, not(target_os = "espidf"))))]
    fn exact_leaf_at(
        &self,
        _name: &OsStr,
        _maximum: u64,
    ) -> Result<Option<BoundCertifiedServePayloadLeaf>, CertifiedServePayloadStoreError> {
        Err(CertifiedServePayloadStoreError::UnsupportedStorageBinding(
            self.expected_path.clone(),
        ))
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    fn leaf_at_exactly_matches(
        &self,
        name: &OsStr,
        maximum: u64,
        expected: &BoundCertifiedServePayloadLeaf,
    ) -> Result<bool, CertifiedServePayloadStoreError> {
        let Some(observed) = self.exact_leaf_at(name, maximum)? else {
            return Ok(false);
        };
        Ok(observed.identity == expected.identity
            && observed.length == expected.length
            && observed.frame_hash == expected.frame_hash)
    }

    #[cfg(not(all(unix, not(target_os = "espidf"))))]
    fn leaf_at_exactly_matches(
        &self,
        _name: &OsStr,
        _maximum: u64,
        _expected: &BoundCertifiedServePayloadLeaf,
    ) -> Result<bool, CertifiedServePayloadStoreError> {
        Err(CertifiedServePayloadStoreError::UnsupportedStorageBinding(
            self.expected_path.clone(),
        ))
    }

    fn create_synced_leaf(
        &self,
        name: &OsStr,
        bytes: &[u8],
        maximum: u64,
    ) -> Result<(File, BoundCertifiedServePayloadLeaf), PersistPayloadError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            use std::os::unix::fs::MetadataExt as _;

            let length = u64::try_from(bytes.len()).map_err(|_| {
                PersistPayloadError::Unpublished(CertifiedServePayloadStoreError::InvalidGeometry(
                    "payload frame length is not representable",
                ))
            })?;
            if length == 0 || length > maximum {
                return Err(PersistPayloadError::Unpublished(
                    CertifiedServePayloadStoreError::EntryTooLarge {
                        actual: length,
                        bound: maximum,
                    },
                ));
            }
            let path = self.expected_path.join(name);
            let descriptor = match rustix::fs::openat(
                &self.directory,
                name,
                rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::CREATE
                    | rustix::fs::OFlags::EXCL
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NONBLOCK,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            ) {
                Ok(descriptor) => descriptor,
                Err(rustix::io::Errno::EXIST) => {
                    return Err(PersistPayloadError::Unpublished(
                        CertifiedServePayloadStoreError::PublicationConflict(path),
                    ));
                }
                Err(error) => {
                    return Err(PersistPayloadError::Unpublished(io_error(
                        "create store leaf",
                        &path,
                        std::io::Error::from(error),
                    )));
                }
            };
            let mut file = File::from(descriptor);
            rustix::fs::fchmod(&file, rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR)
                .map_err(std::io::Error::from)
                .map_err(|source| {
                    PersistPayloadError::PublishedButUnsynchronized(io_error(
                        "set private store leaf mode",
                        &path,
                        source,
                    ))
                })?;
            let created = file.metadata().map_err(|source| {
                PersistPayloadError::PublishedButUnsynchronized(io_error(
                    "inspect created store leaf",
                    &path,
                    source,
                ))
            })?;
            let empty = BoundCertifiedServePayloadLeaf {
                name: name.to_os_string(),
                length: 0,
                frame_hash: None,
                identity: CertifiedServeStorageIdentity::from_metadata(&created),
            };
            if !created.is_file()
                || created.nlink() != 1
                || created.uid() != rustix::process::geteuid().as_raw()
                || created.mode() & 0o7777 != PRIVATE_LEAF_MODE
            {
                return Err(PersistPayloadError::PublishedButUnsynchronized(
                    CertifiedServePayloadStoreError::NonRegularEntry(path),
                ));
            }
            self.verify_open_leaf(&file, &empty)
                .map_err(PersistPayloadError::PublishedButUnsynchronized)?;
            file.write_all(bytes)
                .and_then(|()| file.flush())
                .and_then(|()| file.sync_all())
                .map_err(|source| {
                    PersistPayloadError::PublishedButUnsynchronized(io_error(
                        "synchronise store leaf",
                        &path,
                        source,
                    ))
                })?;
            let leaf = BoundCertifiedServePayloadLeaf {
                length,
                frame_hash: Some(Hash::new(bytes)),
                ..empty
            };
            self.verify_open_leaf(&file, &leaf)
                .and_then(|()| {
                    self.leaf_at_exactly_matches(name, maximum, &leaf)
                        .and_then(|matches| {
                            matches.then_some(()).ok_or_else(|| {
                                CertifiedServePayloadStoreError::PublicationConflict(path)
                            })
                        })
                })
                .map_err(PersistPayloadError::PublishedButUnsynchronized)?;
            Ok((file, leaf))
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = (name, bytes, maximum);
            Err(PersistPayloadError::Unpublished(
                CertifiedServePayloadStoreError::UnsupportedStorageBinding(
                    self.expected_path.clone(),
                ),
            ))
        }
    }

    fn move_leaf_noreplace(
        &self,
        source: &BoundCertifiedServePayloadLeaf,
        destination: &OsStr,
        maximum: u64,
        operation: &'static str,
    ) -> Result<BoundCertifiedServePayloadLeaf, PersistPayloadError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            let destination_path = self.expected_path.join(destination);
            self.validate_publication_destination(destination, maximum, None)
                .map_err(PersistPayloadError::Unpublished)?;
            let file = self
                .open_leaf(source)
                .map_err(PersistPayloadError::Unpublished)?;
            if !self
                .leaf_at_exactly_matches(&source.name, maximum, source)
                .map_err(PersistPayloadError::Unpublished)?
            {
                return Err(PersistPayloadError::Unpublished(
                    CertifiedServePayloadStoreError::PublicationConflict(
                        self.expected_path.join(&source.name),
                    ),
                ));
            }
            if let Err(error) =
                rename_certified_serve_leaf_noreplace(&self.directory, &source.name, destination)
            {
                return Err(PersistPayloadError::Unpublished(io_error(
                    operation,
                    &destination_path,
                    error,
                )));
            }
            let moved = BoundCertifiedServePayloadLeaf {
                name: destination.to_os_string(),
                ..source.clone()
            };
            let verify_move = || {
                self.validate_publication_destination(&source.name, maximum, None)?;
                self.verify_open_leaf(&file, &moved)?;
                if !self.leaf_at_exactly_matches(destination, maximum, &moved)? {
                    return Err(CertifiedServePayloadStoreError::PublicationConflict(
                        destination_path.clone(),
                    ));
                }
                Ok(())
            };
            verify_move().map_err(PersistPayloadError::PublishedButUnsynchronized)?;
            self.sync()
                .map_err(PersistPayloadError::PublishedButUnsynchronized)?;
            verify_move().map_err(PersistPayloadError::PublishedButUnsynchronized)?;
            Ok(moved)
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = (source, destination, maximum, operation);
            Err(PersistPayloadError::Unpublished(
                CertifiedServePayloadStoreError::UnsupportedStorageBinding(
                    self.expected_path.clone(),
                ),
            ))
        }
    }

    fn sync(&self) -> Result<(), CertifiedServePayloadStoreError> {
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            self.verify_linked()?;
            self.directory.sync_all().map_err(|source| {
                io_error("synchronise store directory", &self.expected_path, source)
            })?;
            self.verify_linked()
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            Err(CertifiedServePayloadStoreError::UnsupportedStorageBinding(
                self.expected_path.clone(),
            ))
        }
    }
}

#[cfg(all(
    unix,
    not(target_os = "espidf"),
    any(
        target_vendor = "apple",
        target_os = "linux",
        target_os = "android",
        target_os = "redox"
    )
))]
fn rename_certified_serve_leaf_noreplace(
    directory: &File,
    source: &OsStr,
    destination: &OsStr,
) -> std::io::Result<()> {
    rustix::fs::renameat_with(
        directory,
        source,
        directory,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)
}

#[cfg(all(unix, not(target_os = "espidf")))]
const fn certified_serve_noreplace_supported() -> bool {
    cfg!(any(
        target_vendor = "apple",
        target_os = "linux",
        target_os = "android",
        target_os = "redox"
    ))
}

#[cfg(all(
    unix,
    not(target_os = "espidf"),
    not(any(
        target_vendor = "apple",
        target_os = "linux",
        target_os = "android",
        target_os = "redox"
    ))
))]
fn rename_certified_serve_leaf_noreplace(
    _directory: &File,
    _source: &OsStr,
    _destination: &OsStr,
) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-replace Certified-Serve publication is unavailable",
    ))
}

/// Crash-safe owner of all exact Certified-Serve payload files for one height.
#[derive(Debug)]
pub(crate) struct CertifiedServePayloadStoreV1 {
    identity: Arc<CertifiedServePayloadStoreInstanceIdentityMarker>,
    directory: PathBuf,
    bound_directory: Option<BoundCertifiedServePayloadDirectory>,
    context: wire::HeightContext,
    max_entries: usize,
    max_entry_bytes: u64,
    indexed: BTreeSet<CertifiedServePayloadId>,
    /// Exact open/publication-time terminal snapshots; an ID-only inventory
    /// cannot detect a valid but semantically different companion replacement.
    terminal_companions: BTreeMap<CertifiedServePayloadId, BoundCertifiedServePayloadLeaf>,
    removed: BTreeSet<CertifiedServePayloadId>,
    quarantine: BTreeMap<OsString, BoundCertifiedServePayloadLeaf>,
    /// Emergency Fast owners neither inventory nor mutate retained payload files.
    emergency_read_only: bool,
    #[cfg(test)]
    fail_next_publish_directory_sync: bool,
    #[cfg(test)]
    replace_next_terminal_canonical_before_companion_create: Option<PathBuf>,
    #[cfg(test)]
    race_next_publication_destination_before_noreplace: Option<PathBuf>,
}
#[derive(Debug)]
struct CertifiedServePayloadCensusV1 {
    payloads: BTreeMap<CertifiedServePayloadId, PersistedCertifiedServePayloadV1>,
    terminal_companions: BTreeMap<CertifiedServePayloadId, BoundCertifiedServePayloadLeaf>,
    removed: BTreeSet<CertifiedServePayloadId>,
    quarantine: BTreeMap<OsString, BoundCertifiedServePayloadLeaf>,
}
#[derive(Debug)]
struct CertifiedServePayloadStoreInstanceIdentityMarker;
/// Comparison-only identity for one exact open Serve-payload store instance.
#[derive(Clone, Debug)]
pub(crate) struct CertifiedServePayloadStoreInstanceIdentity(
    Arc<CertifiedServePayloadStoreInstanceIdentityMarker>,
);
impl CertifiedServePayloadStoreInstanceIdentity {
    /// Return whether both seals came from the same open store owner.
    pub(crate) fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl CertifiedServePayloadStoreV1 {
    fn ensure_mutable(&self) -> Result<(), CertifiedServePayloadStoreError> {
        if self.emergency_read_only {
            return Err(CertifiedServePayloadStoreError::EmergencyFastReadOnly);
        }
        Ok(())
    }

    fn bound_directory(
        &self,
    ) -> Result<&BoundCertifiedServePayloadDirectory, CertifiedServePayloadStoreError> {
        self.bound_directory.as_ref().ok_or_else(|| {
            CertifiedServePayloadStoreError::UnsupportedStorageBinding(self.directory.clone())
        })
    }

    /// Project a comparison-only seal before moving this exact store.
    pub(crate) fn instance_identity(&self) -> CertifiedServePayloadStoreInstanceIdentity {
        CertifiedServePayloadStoreInstanceIdentity(Arc::clone(&self.identity))
    }
    /// Reauthenticate the current exact directory for lifecycle finalization.
    ///
    /// Unlike the immutable startup cut, this bounded scan observes mutations
    /// made by live Certified-Serve admission and terminal settlement. The
    /// launch-private permit proves the exact service and store remain coheld
    /// after ingress closure and output handoff. The in-memory index must equal
    /// the strict no-symlink directory census before any payload is trusted.
    pub(in crate::sumeragi) fn authenticate_current_for_lifecycle_retirement(
        &self,
        _permit: super::v2_lifecycle_coordinator::ProductionLifecycleServeRetirementAuthenticationPermitV1,
        verified: &VerifiedHeightContext,
        local_signer: &KeyPair,
    ) -> Result<
        AuthenticatedCertifiedServePayloadRecoveryCut,
        CertifiedServeRetirementAuthenticationErrorV1,
    > {
        if verified.context() != &self.context {
            return Err(CertifiedServePayloadRecoveryError::ForeignContext.into());
        }
        self.ensure_mutable()?;
        let payloads = self.reload_payload_census_strict()?;
        if payloads.keys().copied().collect::<BTreeSet<_>>() != self.indexed {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch.into());
        }
        let authenticated = CertifiedServePayloadRecoveryCut {
            context_id: self.context.id(),
            height: self.context.height,
            payloads,
        }
        .authenticate_for_complete_tip_retirement(verified, local_signer)
        .map_err(CertifiedServeRetirementAuthenticationErrorV1::from)?;
        self.validate_authenticated_cut(&authenticated)?;
        Ok(authenticated)
    }

    /// Compare this opened payload owner with one sealed lifecycle root.
    ///
    /// This fixed oracle exposes neither the root nor the indexed requests.
    pub(in crate::sumeragi) fn matches_lifecycle_storage_root(
        &self,
        root: &Path,
        context: &wire::HeightContext,
    ) -> bool {
        &self.context == context
            && self.directory == root.join(STORE_DIRECTORY)
            && (self.emergency_read_only
                || self
                    .bound_directory
                    .as_ref()
                    .is_some_and(BoundCertifiedServePayloadDirectory::is_linked))
    }
    /// Open one immutable height store from a descriptor-bound authority minted by Kura.
    ///
    /// A fully written terminal stage is resumed only when the immutable
    /// canonical Pending frame binds it exactly. Pending or malformed stages
    /// are moved without replacement into bounded inert quarantine: they never
    /// become authoritative and remain available for operator inspection.
    /// Contradictory stages, symlinks, unknown names, foreign contexts,
    /// oversized files, and non-canonical authoritative frames fail closed.
    ///
    /// # Errors
    ///
    /// Returns an error when geometry cannot be derived or any directory entry
    /// fails the closed storage contract.
    pub(crate) fn open_with_kura_authority(
        kura: &crate::kura::Kura,
        authority: crate::kura::KuraV2CertifiedServePayloadDirectoryAuthority,
        context: &wire::HeightContext,
    ) -> Result<(Self, CertifiedServePayloadRecoveryCut), CertifiedServePayloadStoreError> {
        let max_entries = MAX_CERTIFIED_SERVE_PAYLOADS_PER_HEIGHT;
        let expected_path = kura
            .sumeragi_v2_storage_root()
            .join("lifecycle-v1")
            .join(hex::encode(context.id().0.as_ref()))
            .join(STORE_DIRECTORY);
        let max_entry_bytes = Self::validate_open_parameters(context, max_entries, &expected_path)?;
        let bound_directory =
            BoundCertifiedServePayloadDirectory::from_kura_authority(kura, authority, context)?;
        Self::open_bound(bound_directory, context, max_entries, max_entry_bytes)
    }

    /// Open one immutable-height store from a raw test root.
    ///
    /// Production must consume a recovery-minted authority through
    /// [`Self::open_with_kura_authority`].
    #[cfg(test)]
    pub(crate) fn open(
        root: &Path,
        context: &wire::HeightContext,
    ) -> Result<(Self, CertifiedServePayloadRecoveryCut), CertifiedServePayloadStoreError> {
        Self::open_with_max_entries(root, context, MAX_CERTIFIED_SERVE_PAYLOADS_PER_HEIGHT)
    }
    /// Open an empty, read-only owner without touching the current-height directory.
    pub(crate) fn open_emergency_fast_read_only(
        root: &Path,
        context: &wire::HeightContext,
    ) -> Result<(Self, CertifiedServePayloadRecoveryCut), CertifiedServePayloadStoreError> {
        context
            .validate()
            .map_err(|error| CertifiedServePayloadStoreError::InvalidFrame {
                path: root.to_path_buf(),
                reason: format!("invalid height context: {error}"),
            })?;
        let store = Self {
            identity: Arc::new(CertifiedServePayloadStoreInstanceIdentityMarker),
            directory: root.join(STORE_DIRECTORY),
            bound_directory: None,
            context: context.clone(),
            max_entries: MAX_CERTIFIED_SERVE_PAYLOADS_PER_HEIGHT,
            max_entry_bytes: derive_max_entry_bytes(context)?,
            indexed: BTreeSet::new(),
            terminal_companions: BTreeMap::new(),
            removed: BTreeSet::new(),
            quarantine: BTreeMap::new(),
            emergency_read_only: true,
            #[cfg(test)]
            fail_next_publish_directory_sync: false,
            #[cfg(test)]
            replace_next_terminal_canonical_before_companion_create: None,
            #[cfg(test)]
            race_next_publication_destination_before_noreplace: None,
        };
        let recovery = CertifiedServePayloadRecoveryCut {
            context_id: context.id(),
            height: context.height,
            payloads: BTreeMap::new(),
        };
        Ok((store, recovery))
    }
    /// Open an empty payload owner for a structural lifecycle fixture.
    ///
    /// Production must use [`Self::open_with_kura_authority`]. This skips
    /// wire-context validation only because closed replay-authority fixtures
    /// intentionally use non-cryptographic parent certificates.
    #[cfg(test)]
    pub(in crate::sumeragi) fn open_lifecycle_fixture_for_test(
        root: &Path,
        context: &wire::HeightContext,
    ) -> Result<
        (Self, AuthenticatedCertifiedServePayloadRecoveryCut),
        CertifiedServePayloadStoreError,
    > {
        let directory = root.join(STORE_DIRECTORY);
        let bound_directory = BoundCertifiedServePayloadDirectory::open_or_create(&directory)?;
        Ok((
            Self {
                identity: Arc::new(CertifiedServePayloadStoreInstanceIdentityMarker),
                directory,
                bound_directory: Some(bound_directory),
                context: context.clone(),
                max_entries: MAX_CERTIFIED_SERVE_PAYLOADS_PER_HEIGHT,
                max_entry_bytes: derive_max_entry_bytes(context)?,
                indexed: BTreeSet::new(),
                terminal_companions: BTreeMap::new(),
                removed: BTreeSet::new(),
                quarantine: BTreeMap::new(),
                emergency_read_only: false,
                fail_next_publish_directory_sync: false,
                replace_next_terminal_canonical_before_companion_create: None,
                race_next_publication_destination_before_noreplace: None,
            },
            AuthenticatedCertifiedServePayloadRecoveryCut {
                context_id: context.id(),
                height: context.height,
                payloads: BTreeMap::new(),
            },
        ))
    }
    #[cfg(test)]
    fn open_with_max_entries(
        root: &Path,
        context: &wire::HeightContext,
        max_entries: usize,
    ) -> Result<(Self, CertifiedServePayloadRecoveryCut), CertifiedServePayloadStoreError> {
        let directory = root.join(STORE_DIRECTORY);
        let max_entry_bytes = Self::validate_open_parameters(context, max_entries, &directory)?;
        let bound_directory = BoundCertifiedServePayloadDirectory::open_or_create(&directory)?;
        Self::open_bound(bound_directory, context, max_entries, max_entry_bytes)
    }

    fn validate_open_parameters(
        context: &wire::HeightContext,
        max_entries: usize,
        directory: &Path,
    ) -> Result<u64, CertifiedServePayloadStoreError> {
        if max_entries == 0 || max_entries > MAX_CERTIFIED_SERVE_PAYLOADS_PER_HEIGHT {
            return Err(CertifiedServePayloadStoreError::InvalidGeometry(
                "payload count is zero or exceeds the per-height hard bound",
            ));
        }
        context
            .validate()
            .map_err(|error| CertifiedServePayloadStoreError::InvalidFrame {
                path: directory.to_path_buf(),
                reason: format!("invalid height context: {error}"),
            })?;
        derive_max_entry_bytes(context)
    }

    fn scan_payload_census(
        &self,
        resume_staging: bool,
    ) -> Result<CertifiedServePayloadCensusV1, CertifiedServePayloadStoreError> {
        let traversal_capacity = self
            .max_entries
            .checked_mul(2)
            .and_then(|capacity| capacity.checked_add(MAX_QUARANTINED_STAGES_PER_HEIGHT))
            .and_then(|capacity| capacity.checked_add(MAX_IN_FLIGHT_STAGES_PER_HEIGHT))
            .ok_or(CertifiedServePayloadStoreError::InvalidGeometry(
                "directory traversal capacity overflowed",
            ))?;
        let mut canonicals = BTreeMap::new();
        let mut terminals = BTreeMap::new();
        let mut removals = BTreeMap::new();
        let mut stages = Vec::new();
        let mut quarantine = BTreeMap::new();
        for leaf in self.bound_directory()?.inventory(traversal_capacity)? {
            let path = self.directory.join(&leaf.name);
            let Some(name) = leaf.name.to_str() else {
                return Err(CertifiedServePayloadStoreError::UnexpectedEntry(path));
            };
            #[derive(Clone, Copy)]
            enum EntryKind {
                Canonical,
                Terminal,
                Removal,
                Staging,
                Quarantine,
            }
            let kind = if has_canonical_hash_name(name, TERMINAL_FILE_SUFFIX) {
                EntryKind::Terminal
            } else if has_canonical_hash_name(name, REMOVAL_FILE_SUFFIX) {
                EntryKind::Removal
            } else if quarantine_slot_from_file_name(name).is_some() {
                EntryKind::Quarantine
            } else if has_canonical_hash_name(name, TEMPORARY_FILE_SUFFIX) {
                EntryKind::Staging
            } else if has_canonical_hash_name(name, FILE_SUFFIX) {
                EntryKind::Canonical
            } else {
                return Err(CertifiedServePayloadStoreError::UnexpectedEntry(path));
            };
            if matches!(kind, EntryKind::Staging) {
                if stages.len() >= MAX_IN_FLIGHT_STAGES_PER_HEIGHT {
                    return Err(CertifiedServePayloadStoreError::DirectoryCapacityExceeded {
                        capacity: MAX_IN_FLIGHT_STAGES_PER_HEIGHT,
                    });
                }
                let exact = self
                    .bound_directory()?
                    .exact_leaf_at(&leaf.name, self.max_entry_bytes)?
                    .ok_or_else(|| CertifiedServePayloadStoreError::NonRegularEntry(path))?;
                stages.push(exact);
                continue;
            }
            if matches!(kind, EntryKind::Quarantine) {
                if quarantine.len() >= MAX_QUARANTINED_STAGES_PER_HEIGHT {
                    return Err(CertifiedServePayloadStoreError::DirectoryCapacityExceeded {
                        capacity: MAX_QUARANTINED_STAGES_PER_HEIGHT,
                    });
                }
                let exact = self
                    .bound_directory()?
                    .exact_leaf_at(&leaf.name, self.max_entry_bytes)?
                    .ok_or_else(|| CertifiedServePayloadStoreError::NonRegularEntry(path))?;
                if quarantine.insert(leaf.name, exact).is_some() {
                    return Err(CertifiedServePayloadStoreError::DuplicateRequestHash);
                }
                continue;
            }
            let (payload, exact_leaf) = self.load_leaf_with_bound(&leaf)?;
            let id = payload.id();
            let expected_path = match kind {
                EntryKind::Canonical => self.path_for(id),
                EntryKind::Terminal => self.terminal_path_for(id),
                EntryKind::Removal => self.removal_path_for(id),
                EntryKind::Staging | EntryKind::Quarantine => unreachable!(),
            };
            if expected_path != path {
                return Err(CertifiedServePayloadStoreError::RequestHashFilenameMismatch(path));
            }
            let entries = match kind {
                EntryKind::Canonical => &mut canonicals,
                EntryKind::Terminal => &mut terminals,
                EntryKind::Removal => &mut removals,
                EntryKind::Staging | EntryKind::Quarantine => unreachable!(),
            };
            if entries.insert(id, (payload, path, exact_leaf)).is_some() {
                return Err(CertifiedServePayloadStoreError::DuplicateRequestHash);
            }
        }

        // Validate every authoritative entry before startup moves even an
        // inert stage. Recovery must not mutate around an unrelated poisoned
        // canonical, terminal, or removal state.
        for (id, (canonical, canonical_path, _canonical_leaf)) in &canonicals {
            if removals.contains_key(id) {
                return Err(invalid_frame(
                    canonical_path,
                    "active payload coexists with its removal journal",
                ));
            }
            if !matches!(
                &canonical.state,
                PersistedCertifiedServePayloadStateV1::Pending
            ) {
                return Err(invalid_frame(
                    canonical_path,
                    "canonical payload must remain the immutable Pending frame",
                ));
            }
            if let Some((terminal, terminal_path, _terminal_leaf)) = terminals.get(id)
                && !terminal_companion_matches(canonical, terminal)
            {
                return Err(invalid_frame(
                    terminal_path,
                    "terminal companion does not extend the exact canonical Pending frame",
                ));
            }
        }
        for (id, (_payload, path, _leaf)) in &terminals {
            if !canonicals.contains_key(id) {
                return Err(invalid_frame(
                    path,
                    "terminal companion has no canonical Pending frame",
                ));
            }
        }
        for (id, (payload, path, _leaf)) in &removals {
            if canonicals.contains_key(id) {
                return Err(invalid_frame(
                    path,
                    "removal journal coexists with an active payload",
                ));
            }
            if !matches!(
                &payload.state,
                PersistedCertifiedServePayloadStateV1::Pending
            ) {
                return Err(invalid_frame(
                    path,
                    "removal journal must contain an exact Pending frame",
                ));
            }
        }
        if canonicals
            .len()
            .checked_add(removals.len())
            .is_none_or(|count| count > self.max_entries)
        {
            return Err(CertifiedServePayloadStoreError::PayloadCapacityExceeded {
                capacity: self.max_entries,
            });
        }

        if let Some(stage) = stages.into_iter().next() {
            let stage_path = self.directory.join(&stage.name);
            if !resume_staging {
                return Err(invalid_frame(
                    &stage_path,
                    "staging appeared behind the retained store owner",
                ));
            }
            match self.load_leaf_with_bound(&stage) {
                Ok((staged, exact_stage))
                    if !matches!(
                        &staged.state,
                        PersistedCertifiedServePayloadStateV1::Pending
                    ) =>
                {
                    let id = staged.id();
                    if exact_stage.name != self.temporary_file_name_for(id) {
                        return Err(
                            CertifiedServePayloadStoreError::RequestHashFilenameMismatch(
                                stage_path,
                            ),
                        );
                    }
                    let Some((canonical, _, _canonical_leaf)) = canonicals.get(&id) else {
                        return Err(invalid_frame(
                            &stage_path,
                            "terminal staging has no canonical Pending frame",
                        ));
                    };
                    if terminals.contains_key(&id)
                        || removals.contains_key(&id)
                        || !terminal_companion_matches(canonical, &staged)
                    {
                        return Err(invalid_frame(
                            &stage_path,
                            "terminal staging contradicts the durable payload state",
                        ));
                    }
                    self.bound_directory()?
                        .move_leaf_noreplace(
                            &exact_stage,
                            &self.terminal_file_name_for(id),
                            self.max_entry_bytes,
                            "resume terminal companion publication",
                        )
                        .map_err(PersistPayloadError::into_store_error)?;
                    return self.scan_payload_census(false);
                }
                Ok((_staged_pending, exact_stage)) => {
                    self.quarantine_stage(&exact_stage, &quarantine)?;
                    return self.scan_payload_census(false);
                }
                Err(
                    CertifiedServePayloadStoreError::InvalidFrame { .. }
                    | CertifiedServePayloadStoreError::ForeignContext(_),
                ) => {
                    self.quarantine_stage(&stage, &quarantine)?;
                    return self.scan_payload_census(false);
                }
                Err(error) => return Err(error),
            }
        }

        let mut payloads = BTreeMap::new();
        let mut terminal_companions = BTreeMap::new();
        for (id, (canonical, canonical_path, _canonical_leaf)) in canonicals {
            if removals.contains_key(&id) {
                return Err(invalid_frame(
                    &canonical_path,
                    "active payload coexists with its removal journal",
                ));
            }
            if !matches!(
                &canonical.state,
                PersistedCertifiedServePayloadStateV1::Pending
            ) {
                return Err(invalid_frame(
                    &canonical_path,
                    "canonical payload must remain the immutable Pending frame",
                ));
            }
            let logical =
                if let Some((terminal, terminal_path, terminal_leaf)) = terminals.remove(&id) {
                    if !terminal_companion_matches(&canonical, &terminal) {
                        return Err(invalid_frame(
                            &terminal_path,
                            "terminal companion does not extend the exact canonical Pending frame",
                        ));
                    }
                    terminal_companions.insert(id, terminal_leaf);
                    terminal
                } else {
                    canonical
                };
            if payloads.insert(id, logical).is_some() {
                return Err(CertifiedServePayloadStoreError::DuplicateRequestHash);
            }
        }
        if let Some((_id, (_payload, path, _leaf))) = terminals.into_iter().next() {
            return Err(invalid_frame(
                &path,
                "terminal companion has no canonical Pending frame",
            ));
        }

        let mut removed = BTreeSet::new();
        for (id, (payload, path, _leaf)) in removals {
            if !matches!(
                &payload.state,
                PersistedCertifiedServePayloadStateV1::Pending
            ) {
                return Err(invalid_frame(
                    &path,
                    "removal journal must contain an exact Pending frame",
                ));
            }
            if !removed.insert(id) {
                return Err(CertifiedServePayloadStoreError::DuplicateRequestHash);
            }
        }
        if payloads
            .len()
            .checked_add(removed.len())
            .is_none_or(|count| count > self.max_entries)
        {
            return Err(CertifiedServePayloadStoreError::PayloadCapacityExceeded {
                capacity: self.max_entries,
            });
        }
        Ok(CertifiedServePayloadCensusV1 {
            payloads,
            terminal_companions,
            removed,
            quarantine,
        })
    }

    fn quarantine_stage(
        &self,
        stage: &BoundCertifiedServePayloadLeaf,
        existing_quarantine: &BTreeMap<OsString, BoundCertifiedServePayloadLeaf>,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        if existing_quarantine.len() >= MAX_QUARANTINED_STAGES_PER_HEIGHT {
            return Err(CertifiedServePayloadStoreError::DirectoryCapacityExceeded {
                capacity: MAX_QUARANTINED_STAGES_PER_HEIGHT,
            });
        }
        // Monotonic ordinal slots let repeated ordinary crashes preserve every
        // inert stage without overwrite or unlink. The closed parser and the
        // global physical cap keep traversal and recovery work bounded.
        let quarantine_name = (0..MAX_QUARANTINED_STAGES_PER_HEIGHT)
            .filter_map(|slot| quarantine_file_name_for_stage(&stage.name, slot))
            .find(|name| !existing_quarantine.contains_key(name))
            .ok_or_else(
                || CertifiedServePayloadStoreError::DirectoryCapacityExceeded {
                    capacity: MAX_QUARANTINED_STAGES_PER_HEIGHT,
                },
            )?;
        self.bound_directory()?
            .move_leaf_noreplace(
                stage,
                &quarantine_name,
                self.max_entry_bytes,
                "quarantine interrupted payload staging",
            )
            .map_err(PersistPayloadError::into_store_error)?;
        Ok(())
    }

    fn open_bound(
        bound_directory: BoundCertifiedServePayloadDirectory,
        context: &wire::HeightContext,
        max_entries: usize,
        max_entry_bytes: u64,
    ) -> Result<(Self, CertifiedServePayloadRecoveryCut), CertifiedServePayloadStoreError> {
        let directory = bound_directory.expected_path.clone();
        #[cfg(all(unix, not(target_os = "espidf")))]
        if !certified_serve_noreplace_supported() {
            return Err(CertifiedServePayloadStoreError::UnsupportedStorageBinding(
                directory.clone(),
            ));
        }
        let mut store = Self {
            identity: Arc::new(CertifiedServePayloadStoreInstanceIdentityMarker),
            directory,
            bound_directory: Some(bound_directory),
            context: context.clone(),
            max_entries,
            max_entry_bytes,
            indexed: BTreeSet::new(),
            terminal_companions: BTreeMap::new(),
            removed: BTreeSet::new(),
            quarantine: BTreeMap::new(),
            emergency_read_only: false,
            #[cfg(test)]
            fail_next_publish_directory_sync: false,
            #[cfg(test)]
            replace_next_terminal_canonical_before_companion_create: None,
            #[cfg(test)]
            race_next_publication_destination_before_noreplace: None,
        };
        let census = store.scan_payload_census(true)?;
        store.indexed.extend(census.payloads.keys().copied());
        store.terminal_companions = census.terminal_companions;
        store.removed = census.removed;
        store.quarantine = census.quarantine;
        let recovery = CertifiedServePayloadRecoveryCut {
            context_id: context.id(),
            height: context.height,
            payloads: census.payloads,
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
        let publication = self
            .retain_for_admission_with_verified_retention(verified, local_signer, authenticated)
            .map_err(CertifiedServePayloadRetentionError::into_store_error)?;
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
    ) -> Result<DurableCertifiedServeAdmissionPublication, CertifiedServePayloadRetentionError>
    {
        let local_validator = self
            .verified_local_retainer(verified, local_signer, authenticated)
            .map_err(CertifiedServePayloadRetentionError::Unchanged)?;
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
        let publication = self
            .retain_for_admission_inner(authenticated, local_retainer)
            .map_err(CertifiedServePayloadRetentionError::into_store_error)?;
        publication
            .is_pending()
            .then_some(publication.receipt())
            .ok_or(CertifiedServePayloadStoreError::TerminalResurrection)
    }
    fn retain_for_admission_inner(
        &mut self,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        local_retainer: wire::ValidatorIndex,
    ) -> Result<DurableCertifiedServeAdmissionPublication, CertifiedServePayloadRetentionError>
    {
        self.ensure_mutable()
            .map_err(CertifiedServePayloadRetentionError::Unchanged)?;
        let request = authenticated.request();
        let id = CertifiedServePayloadId::from_request(request);
        if id.request_hash() != authenticated.request_hash() {
            return Err(CertifiedServePayloadRetentionError::Unchanged(
                CertifiedServePayloadStoreError::AuthenticatedRequestHashMismatch,
            ));
        }
        self.validate_request(request, &self.directory)
            .map_err(CertifiedServePayloadRetentionError::Unchanged)?;
        if self.indexed.contains(&id) {
            let existing = self
                .load_id(id)
                .map_err(CertifiedServePayloadRetentionError::Unchanged)?;
            if existing.request != *request {
                return Err(CertifiedServePayloadRetentionError::Unchanged(
                    CertifiedServePayloadStoreError::RequestHashCollision,
                ));
            }
            let state = match &existing.state {
                PersistedCertifiedServePayloadStateV1::Pending => {
                    DurableCertifiedServeAdmissionStateV1::Pending
                }
                PersistedCertifiedServePayloadStateV1::Completed { response_hash, .. } => {
                    DurableCertifiedServeAdmissionStateV1::Completed(*response_hash)
                }
                PersistedCertifiedServePayloadStateV1::Negative { outcome } => {
                    DurableCertifiedServeAdmissionStateV1::Negative(*outcome)
                }
            };
            return Ok(DurableCertifiedServeAdmissionPublication {
                receipt: admission_receipt(&existing, local_retainer),
                state,
                fresh_pending: false,
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
        if self.removed.contains(&id) {
            self.restore_removed_pending(&payload)
                .map_err(PersistPayloadError::into_retention_error)?;
            return Ok(DurableCertifiedServeAdmissionPublication {
                receipt,
                state: DurableCertifiedServeAdmissionStateV1::Pending,
                fresh_pending: true,
            });
        }
        if self
            .indexed
            .len()
            .checked_add(self.removed.len())
            .is_none_or(|count| count >= self.max_entries)
        {
            return Err(CertifiedServePayloadRetentionError::Unchanged(
                CertifiedServePayloadStoreError::PayloadCapacityExceeded {
                    capacity: self.max_entries,
                },
            ));
        }
        self.persist_payload(&payload, None)
            .map_err(PersistPayloadError::into_retention_error)?;
        self.indexed.insert(id);
        Ok(DurableCertifiedServeAdmissionPublication {
            receipt,
            state: DurableCertifiedServeAdmissionStateV1::Pending,
            fresh_pending: true,
        })
    }

    fn restore_removed_pending(
        &mut self,
        expected: &PersistedCertifiedServePayloadV1,
    ) -> Result<(), PersistPayloadError> {
        let id = expected.id();
        if !self.removed.contains(&id)
            || self.indexed.contains(&id)
            || self.terminal_companions.contains_key(&id)
        {
            return Err(PersistPayloadError::Unpublished(
                CertifiedServePayloadStoreError::PendingRollbackMismatch,
            ));
        }
        let before = self
            .reload_payload_census_strict()
            .map_err(PersistPayloadError::Unpublished)?;
        if before.keys().copied().collect::<BTreeSet<_>>() != self.indexed {
            return Err(PersistPayloadError::Unpublished(
                CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch,
            ));
        }
        let removal_name = self.removal_file_name_for(id);
        let removal_path = self.removal_path_for(id);
        let leaf = self
            .bound_directory()
            .map_err(PersistPayloadError::Unpublished)?
            .inspect_leaf(&removal_name, self.max_entry_bytes)
            .map_err(PersistPayloadError::Unpublished)?
            .ok_or_else(|| {
                PersistPayloadError::Unpublished(
                    CertifiedServePayloadStoreError::PendingRollbackMismatch,
                )
            })?;
        let (removed, leaf) = self
            .load_leaf_with_bound(&leaf)
            .map_err(PersistPayloadError::Unpublished)?;
        if &removed != expected || self.removal_path_for(removed.id()) != removal_path {
            return Err(PersistPayloadError::Unpublished(
                CertifiedServePayloadStoreError::PendingRollbackMismatch,
            ));
        }
        self.bound_directory()
            .map_err(PersistPayloadError::Unpublished)?
            .validate_publication_destination(
                &self.terminal_file_name_for(id),
                self.max_entry_bytes,
                None,
            )
            .map_err(PersistPayloadError::Unpublished)?;
        self.bound_directory()
            .map_err(PersistPayloadError::Unpublished)?
            .move_leaf_noreplace(
                &leaf,
                &self.file_name_for(id),
                self.max_entry_bytes,
                "reactivate removed Pending payload",
            )?;
        let (observed, _leaf, has_terminal) = self
            .load_id_with_leaf_untracked_terminal(id)
            .map_err(PersistPayloadError::PublishedButUnsynchronized)?;
        if &observed != expected || has_terminal.is_some() {
            return Err(PersistPayloadError::PublishedButUnsynchronized(
                CertifiedServePayloadStoreError::PendingRollbackMismatch,
            ));
        }
        let after = self
            .scan_payload_census(false)
            .map_err(PersistPayloadError::PublishedButUnsynchronized)?;
        let mut expected_ids = self.indexed.clone();
        expected_ids.insert(id);
        let mut expected_removed = self.removed.clone();
        expected_removed.remove(&id);
        if after.payloads.keys().copied().collect::<BTreeSet<_>>() != expected_ids
            || after.payloads.get(&id) != Some(expected)
            || after.terminal_companions != self.terminal_companions
            || after.removed != expected_removed
            || after.quarantine != self.quarantine
        {
            return Err(PersistPayloadError::PublishedButUnsynchronized(
                CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch,
            ));
        }
        let removed_tracking = self.removed.remove(&id);
        let active_tracking = self.indexed.insert(id);
        debug_assert!(removed_tracking && active_tracking);
        Ok(())
    }

    fn journal_removed_pending(
        &mut self,
        expected: &PersistedCertifiedServePayloadV1,
        leaf: &BoundCertifiedServePayloadLeaf,
    ) -> Result<(), PersistPayloadError> {
        let id = expected.id();
        if !matches!(
            &expected.state,
            PersistedCertifiedServePayloadStateV1::Pending
        ) || !self.indexed.contains(&id)
            || self.removed.contains(&id)
            || self.terminal_companions.contains_key(&id)
        {
            return Err(PersistPayloadError::Unpublished(
                CertifiedServePayloadStoreError::PendingRollbackMismatch,
            ));
        }
        let before = self
            .reload_payload_census_strict()
            .map_err(PersistPayloadError::Unpublished)?;
        if before.keys().copied().collect::<BTreeSet<_>>() != self.indexed
            || before.get(&id) != Some(expected)
        {
            return Err(PersistPayloadError::Unpublished(
                CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch,
            ));
        }
        let terminal_name = self.terminal_file_name_for(id);
        self.bound_directory()
            .map_err(PersistPayloadError::Unpublished)?
            .validate_publication_destination(&terminal_name, self.max_entry_bytes, None)
            .map_err(PersistPayloadError::Unpublished)?;
        let removal_name = self.removal_file_name_for(id);
        let moved = self
            .bound_directory()
            .map_err(PersistPayloadError::Unpublished)?
            .move_leaf_noreplace(
                leaf,
                &removal_name,
                self.max_entry_bytes,
                "journal removed Pending payload",
            )?;
        let (removed, moved) = self
            .load_leaf_with_bound(&moved)
            .map_err(PersistPayloadError::PublishedButUnsynchronized)?;
        if &removed != expected
            || moved.name != removal_name
            || self
                .bound_directory()
                .map_err(PersistPayloadError::PublishedButUnsynchronized)?
                .inspect_leaf(&self.file_name_for(id), self.max_entry_bytes)
                .map_err(PersistPayloadError::PublishedButUnsynchronized)?
                .is_some()
            || self
                .bound_directory()
                .map_err(PersistPayloadError::PublishedButUnsynchronized)?
                .inspect_leaf(&terminal_name, self.max_entry_bytes)
                .map_err(PersistPayloadError::PublishedButUnsynchronized)?
                .is_some()
        {
            return Err(PersistPayloadError::PublishedButUnsynchronized(
                CertifiedServePayloadStoreError::PendingRollbackMismatch,
            ));
        }
        let after = self
            .scan_payload_census(false)
            .map_err(PersistPayloadError::PublishedButUnsynchronized)?;
        let mut expected_ids = self.indexed.clone();
        expected_ids.remove(&id);
        let mut expected_removed = self.removed.clone();
        expected_removed.insert(id);
        if after.payloads.keys().copied().collect::<BTreeSet<_>>() != expected_ids
            || after.terminal_companions != self.terminal_companions
            || after.removed != expected_removed
            || after.quarantine != self.quarantine
        {
            return Err(PersistPayloadError::PublishedButUnsynchronized(
                CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch,
            ));
        }
        let active_tracking = self.indexed.remove(&id);
        let removed_tracking = self.removed.insert(id);
        debug_assert!(active_tracking && removed_tracking);
        Ok(())
    }
    /// Remove an exact pending publication after admission conclusively
    /// declined it.
    ///
    /// This is the compensating half of the payload-first/ledger-second
    /// admission transaction. The sealed receipt must still name the exact
    /// pending frame; terminal material is never removed through this path.
    /// The immutable Pending frame is moved into its authenticated removal
    /// journal and synchronised before the in-memory index is released.
    pub(super) fn rollback_pending(
        &mut self,
        receipt: DurableCertifiedServeAdmissionReceipt,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        self.rollback_pending_batch(&[receipt])
    }
    /// Remove one exact batch of pending publications at a typed rollover cut.
    ///
    /// Every receipt is reloaded and validated before the first journal move. A
    /// partial filesystem failure therefore requires restart, but can never
    /// remove terminal evidence or a publication outside the supplied batch.
    pub(super) fn rollback_pending_batch(
        &mut self,
        receipts: &[DurableCertifiedServeAdmissionReceipt],
    ) -> Result<(), CertifiedServePayloadStoreError> {
        self.ensure_mutable()?;
        let mut ids = BTreeSet::new();
        let mut pending = BTreeMap::new();
        for receipt in receipts {
            let id = receipt.id();
            if !ids.insert(id) || !self.indexed.contains(&id) {
                return Err(CertifiedServePayloadStoreError::PendingRollbackMismatch);
            }
            let (payload, leaf) = self.load_id_with_leaf(id)?;
            if !matches!(
                &payload.state,
                PersistedCertifiedServePayloadStateV1::Pending
            ) || payload.payload_hash() != receipt.payload_hash()
                || HashOf::new(&payload.request.certificate) != receipt.certificate_hash()
            {
                return Err(CertifiedServePayloadStoreError::PendingRollbackMismatch);
            }
            pending.insert(id, (payload, leaf));
        }
        for id in &ids {
            let (payload, leaf) = pending
                .get(id)
                .expect("validated rollback id owns one exact bound leaf");
            self.journal_removed_pending(payload, leaf)
                .map_err(PersistPayloadError::into_store_error)?;
        }
        Ok(())
    }
    /// Prune every fully authenticated payload that has no lifecycle-ledger
    /// owner after restart reconciliation succeeds.
    ///
    /// The authenticated cut must cover the store's complete current index and
    /// every retained identity must occur in that cut. Each frame is reloaded
    /// and matched to the cut before any removal-journal move, preventing a
    /// stale recovery snapshot from retiring newly published work.
    pub(super) fn prune_authenticated_orphans(
        &mut self,
        authenticated: &mut AuthenticatedCertifiedServePayloadRecoveryCut,
        retained: &BTreeSet<CertifiedServePayloadId>,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        self.ensure_mutable()?;
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
        let mut pending = BTreeMap::new();
        for id in &orphans {
            let (payload, leaf) = self.load_id_with_leaf(*id)?;
            if !matches!(
                &payload.state,
                PersistedCertifiedServePayloadStateV1::Pending
            ) {
                return Err(CertifiedServePayloadStoreError::OrphanTerminalPayload);
            }
            pending.insert(*id, (payload, leaf));
        }
        for id in &orphans {
            let (payload, leaf) = pending
                .get(id)
                .expect("validated orphan owns one exact Pending leaf");
            self.journal_removed_pending(payload, leaf)
                .map_err(PersistPayloadError::into_store_error)?;
        }
        authenticated.retain_owned(retained);
        self.validate_authenticated_cut(authenticated)
    }
    fn reload_payload_census_strict(
        &self,
    ) -> Result<
        BTreeMap<CertifiedServePayloadId, PersistedCertifiedServePayloadV1>,
        CertifiedServePayloadStoreError,
    > {
        let census = self.scan_payload_census(false)?;
        if census.terminal_companions != self.terminal_companions
            || census.removed != self.removed
            || census.quarantine != self.quarantine
        {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch);
        }
        Ok(census.payloads)
    }
    /// Verify that a post-authentication startup cut still covers the complete
    /// durable directory byte-for-byte.
    ///
    /// Validation rescans the bounded canonical directory instead of trusting
    /// the index captured at open, so a second writer cannot add an otherwise
    /// valid payload behind the retained owner and escape the exact census.
    pub(super) fn validate_authenticated_cut(
        &self,
        authenticated: &AuthenticatedCertifiedServePayloadRecoveryCut,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        if authenticated.context_id() != self.context.id()
            || authenticated.height() != self.context.height
        {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch);
        }
        if self.emergency_read_only {
            return (self.indexed.is_empty() && authenticated.payloads.is_empty())
                .then_some(())
                .ok_or(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch);
        }
        let cut_ids = authenticated
            .iter()
            .map(AuthenticatedRecoveredCertifiedServePayload::id)
            .collect::<BTreeSet<_>>();
        let observed = self.reload_payload_census_strict()?;
        let observed_ids = observed.keys().copied().collect::<BTreeSet<_>>();
        if observed_ids != self.indexed || cut_ids != observed_ids {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch);
        }
        for recovered in authenticated.iter() {
            let payload = observed
                .get(&recovered.id())
                .ok_or(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch)?;
            if !recovered.exactly_matches_persisted_payload()
                || payload.payload_hash() != recovered.payload_hash()
            {
                return Err(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch);
            }
        }
        Ok(())
    }
    /// Retire every retained Pending payload and return the exact refreshed cut.
    ///
    /// The complete pre-mutation cut is revalidated first. Unowned payloads
    /// may be removed only when they are Pending, then every retained Pending
    /// frame is durably replaced by `Cancelled` in canonical request-hash
    /// order. Each in-memory entry advances only through the receipt returned
    /// by that exact write, so callers never fabricate authenticated terminal
    /// payload state after mutation.
    pub(super) fn retire_authenticated_cut(
        &mut self,
        mut authenticated: AuthenticatedCertifiedServePayloadRecoveryCut,
        retained: &BTreeSet<CertifiedServePayloadId>,
    ) -> Result<AuthenticatedCertifiedServePayloadRecoveryCut, CertifiedServeTerminalPersistenceError>
    {
        self.ensure_mutable()
            .map_err(CertifiedServeTerminalPersistenceError::StoreInvariant)?;
        self.validate_authenticated_cut(&authenticated)
            .map_err(CertifiedServeTerminalPersistenceError::StoreInvariant)?;
        self.prune_authenticated_orphans(&mut authenticated, retained)
            .map_err(CertifiedServeTerminalPersistenceError::StoreInvariant)?;
        let pending = authenticated
            .iter()
            .filter(|payload| {
                matches!(
                    payload.state(),
                    AuthenticatedRecoveredCertifiedServePayloadState::Pending
                )
            })
            .map(AuthenticatedRecoveredCertifiedServePayload::id)
            .collect::<Vec<_>>();
        for id in pending {
            let request = authenticated
                .get(id)
                .expect("pending id came from the authenticated cut")
                .request()
                .clone();
            let expected_certificate = authenticated
                .get(id)
                .expect("pending id came from the authenticated cut")
                .certificate_hash();
            let receipt = self.persist_negative_for_authenticated_request(
                &request,
                CertifiedServePayloadNegativeOutcome::Cancelled,
            )?;
            if receipt.id() != id
                || receipt.certificate_hash() != expected_certificate
                || receipt.outcome() != CertifiedServePayloadNegativeOutcome::Cancelled
            {
                return Err(CertifiedServeTerminalPersistenceError::StoreInvariant(
                    CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch,
                ));
            }
            let refreshed = authenticated
                .payloads
                .get_mut(&id)
                .expect("pending id remains in the retained cut");
            if !matches!(
                refreshed.state,
                AuthenticatedRecoveredCertifiedServePayloadState::Pending
            ) {
                return Err(CertifiedServeTerminalPersistenceError::StoreInvariant(
                    CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch,
                ));
            }
            refreshed.state = AuthenticatedRecoveredCertifiedServePayloadState::Negative(
                CertifiedServePayloadNegativeOutcome::Cancelled,
            );
            refreshed.payload_hash = receipt.payload_hash();
        }
        self.validate_authenticated_cut(&authenticated)
            .map_err(CertifiedServeTerminalPersistenceError::StoreInvariant)?;
        Ok(authenticated)
    }
    /// Test-only convenience for synthetic responses that have no canonical
    /// `SignedBlockWire` body. Production completion uses only
    /// [`Self::persist_completed_with_exact_body`].
    #[cfg(test)]
    pub(crate) fn persist_completed(
        &mut self,
        authenticated_request: &AuthenticatedCertifiedBodyRequest,
        response: &wire::CertifiedBodyResponse,
    ) -> Result<DurableCertifiedServeCompletedReceipt, CertifiedServePayloadStoreError> {
        if authenticated_request.request_hash() != response.request_hash {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRequestHashMismatch);
        }
        self.validate_completed_response(authenticated_request.request(), response)?;
        self.persist_completed_response(authenticated_request.request(), response)
            .map_err(CertifiedServeTerminalPersistenceError::into_store_error)
    }
    /// Persist completion through one caller-retained exact body-store owner.
    ///
    /// This is the sole production completion writer. It reloads canonical
    /// bytes through the caller-retained exact body-store owner; the synthetic
    /// two-argument test helper never crosses the lifecycle-owner boundary.
    #[cfg(any(not(test), feature = "bls"))]
    pub(super) fn persist_completed_with_exact_body(
        &mut self,
        authenticated_request: &AuthenticatedCertifiedBodyRequest,
        durable_body: &DurableBodyReceipt,
        body_store: &V2BodyStore,
        response: &wire::CertifiedBodyResponse,
    ) -> Result<DurableCertifiedServeCompletedReceipt, CertifiedServeTerminalPersistenceError> {
        if authenticated_request.request_hash() != response.request_hash {
            return Err(CertifiedServeTerminalPersistenceError::InputRejected(
                CertifiedServePayloadStoreError::AuthenticatedRequestHashMismatch,
            ));
        }
        if let Err(error) = self.validate_durable_response_body(
            authenticated_request.request(),
            durable_body,
            body_store,
            response,
        ) {
            return Err(
                if matches!(
                    &error,
                    CertifiedServePayloadStoreError::DurableBodyReceiptMismatch
                        | CertifiedServePayloadStoreError::DurableResponseBodyMismatch
                ) {
                    CertifiedServeTerminalPersistenceError::InputRejected(error)
                } else {
                    CertifiedServeTerminalPersistenceError::StoreInvariant(error)
                },
            );
        }
        self.validate_completed_response(authenticated_request.request(), response)
            .map_err(CertifiedServeTerminalPersistenceError::InputRejected)?;
        self.persist_completed_response(authenticated_request.request(), response)
    }
    /// Persist completion from one exact worker-owned body-store readback.
    ///
    /// Launch moves the body store into the I/O worker and leaves only its
    /// comparison seal in the lifecycle owner. The opaque readback proves the
    /// worker loaded the exact durable frame from that same store instance;
    /// this method consumes it before publishing terminal response metadata.
    #[cfg(any(not(test), feature = "bls"))]
    pub(super) fn persist_completed_with_worker_readback(
        &mut self,
        authenticated_request: &AuthenticatedCertifiedBodyRequest,
        body_readback: DurableCertifiedServeBodyReadbackV1,
        expected_body_store: &V2BodyStoreInstanceIdentity,
        response: &wire::CertifiedBodyResponse,
    ) -> Result<DurableCertifiedServeCompletedReceipt, CertifiedServeTerminalPersistenceError> {
        if authenticated_request.request_hash() != response.request_hash {
            return Err(CertifiedServeTerminalPersistenceError::InputRejected(
                CertifiedServePayloadStoreError::AuthenticatedRequestHashMismatch,
            ));
        }
        if let Err(error) = self.validate_worker_readback_response_body(
            authenticated_request.request(),
            &body_readback,
            expected_body_store,
            response,
        ) {
            return Err(
                if matches!(
                    &error,
                    CertifiedServePayloadStoreError::DurableBodyReceiptMismatch
                        | CertifiedServePayloadStoreError::DurableResponseBodyMismatch
                ) {
                    CertifiedServeTerminalPersistenceError::InputRejected(error)
                } else {
                    CertifiedServeTerminalPersistenceError::StoreInvariant(error)
                },
            );
        }
        self.validate_completed_response(authenticated_request.request(), response)
            .map_err(CertifiedServeTerminalPersistenceError::InputRejected)?;
        self.persist_completed_response(authenticated_request.request(), response)
    }
    fn persist_completed_response(
        &mut self,
        authenticated_request: &wire::CertifiedBodyRequest,
        response: &wire::CertifiedBodyResponse,
    ) -> Result<DurableCertifiedServeCompletedReceipt, CertifiedServeTerminalPersistenceError> {
        let id = CertifiedServePayloadId(response.request_hash);
        if !self.indexed.contains(&id) {
            return Err(CertifiedServeTerminalPersistenceError::StoreInvariant(
                CertifiedServePayloadStoreError::UnknownPayload,
            ));
        }
        let (mut payload, incumbent) = self
            .load_id_with_leaf(id)
            .map_err(CertifiedServeTerminalPersistenceError::StoreInvariant)?;
        if payload.request != *authenticated_request {
            return Err(CertifiedServeTerminalPersistenceError::StoreInvariant(
                CertifiedServePayloadStoreError::AuthenticatedRequestHashMismatch,
            ));
        }
        let responder = self
            .context
            .roster
            .iter()
            .position(|entry| entry.validator == response.responder)
            .and_then(|index| wire::ValidatorIndex::try_from(index).ok())
            .ok_or_else(|| {
                CertifiedServeTerminalPersistenceError::InputRejected(invalid_frame(
                    &self.directory,
                    "response signer is outside the frozen roster",
                ))
            })?;
        let completed = PersistedCertifiedServePayloadStateV1::Completed {
            response_hash: HashOf::new(response),
            manifest: response.manifest.clone(),
            responder,
            signature: response.signature.clone(),
        };
        match &payload.state {
            PersistedCertifiedServePayloadStateV1::Pending => {}
            existing @ PersistedCertifiedServePayloadStateV1::Completed { .. }
                if existing == &completed =>
            {
                return completed_receipt(&payload)
                    .map_err(CertifiedServeTerminalPersistenceError::StoreInvariant);
            }
            PersistedCertifiedServePayloadStateV1::Completed { .. }
            | PersistedCertifiedServePayloadStateV1::Negative { .. } => {
                return Err(CertifiedServeTerminalPersistenceError::StoreInvariant(
                    CertifiedServePayloadStoreError::TerminalConflict,
                ));
            }
        }
        payload.state = completed;
        let receipt = completed_receipt(&payload)
            .map_err(CertifiedServeTerminalPersistenceError::StoreInvariant)?;
        self.persist_payload(&payload, Some(&incumbent))
            .map_err(PersistPayloadError::into_terminal_error)?;
        Ok(receipt)
    }
    /// Test one deterministic negative terminal result from a synthetic
    /// request-id fixture.
    ///
    /// Exact repeats are idempotent; neither a different negative tag nor a
    /// completed response can replace a durable terminal result.
    ///
    /// # Errors
    ///
    /// Returns an error when `id` is unknown, a conflicting terminal result
    /// exists, or persistence fails.
    #[cfg(test)]
    pub(crate) fn persist_negative(
        &mut self,
        id: CertifiedServePayloadId,
        outcome: CertifiedServePayloadNegativeOutcome,
    ) -> Result<DurableCertifiedServeNegativeReceipt, CertifiedServePayloadStoreError> {
        self.persist_negative_inner(id, outcome)
            .map_err(CertifiedServeTerminalPersistenceError::into_store_error)
    }
    fn persist_negative_inner(
        &mut self,
        id: CertifiedServePayloadId,
        outcome: CertifiedServePayloadNegativeOutcome,
    ) -> Result<DurableCertifiedServeNegativeReceipt, CertifiedServeTerminalPersistenceError> {
        if !self.indexed.contains(&id) {
            return Err(CertifiedServeTerminalPersistenceError::StoreInvariant(
                CertifiedServePayloadStoreError::UnknownPayload,
            ));
        }
        let (mut payload, incumbent) = self
            .load_id_with_leaf(id)
            .map_err(CertifiedServeTerminalPersistenceError::StoreInvariant)?;
        let negative = PersistedCertifiedServePayloadStateV1::Negative { outcome };
        match &payload.state {
            PersistedCertifiedServePayloadStateV1::Pending => {}
            existing @ PersistedCertifiedServePayloadStateV1::Negative { .. }
                if existing == &negative =>
            {
                return negative_receipt(&payload)
                    .map_err(CertifiedServeTerminalPersistenceError::StoreInvariant);
            }
            PersistedCertifiedServePayloadStateV1::Completed { .. }
            | PersistedCertifiedServePayloadStateV1::Negative { .. } => {
                return Err(CertifiedServeTerminalPersistenceError::StoreInvariant(
                    CertifiedServePayloadStoreError::TerminalConflict,
                ));
            }
        }
        payload.state = negative;
        let receipt = negative_receipt(&payload)
            .map_err(CertifiedServeTerminalPersistenceError::StoreInvariant)?;
        self.persist_payload(&payload, Some(&incumbent))
            .map_err(PersistPayloadError::into_terminal_error)?;
        Ok(receipt)
    }
    /// Persist a negative result for the exact authenticated request.
    ///
    /// The opaque payload id is derived inside this store. Callers cannot
    /// splice an authenticated request onto a separately supplied id.
    pub(super) fn persist_negative_for_authenticated_request(
        &mut self,
        authenticated_request: &AuthenticatedCertifiedBodyRequest,
        outcome: CertifiedServePayloadNegativeOutcome,
    ) -> Result<DurableCertifiedServeNegativeReceipt, CertifiedServeTerminalPersistenceError> {
        let request = authenticated_request.request();
        if authenticated_request.request_hash() != HashOf::new(request) {
            return Err(CertifiedServeTerminalPersistenceError::StoreInvariant(
                CertifiedServePayloadStoreError::AuthenticatedRequestHashMismatch,
            ));
        }
        let id = CertifiedServePayloadId::from_request(request);
        if !self.indexed.contains(&id) {
            return Err(CertifiedServeTerminalPersistenceError::StoreInvariant(
                CertifiedServePayloadStoreError::UnknownPayload,
            ));
        }
        let payload = self
            .load_id(id)
            .map_err(CertifiedServeTerminalPersistenceError::StoreInvariant)?;
        if payload.request != *request {
            return Err(CertifiedServeTerminalPersistenceError::StoreInvariant(
                CertifiedServePayloadStoreError::AuthenticatedRequestHashMismatch,
            ));
        }
        self.persist_negative_inner(id, outcome)
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
        if !body_store.owns_receipt(durable_body) {
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
    #[cfg(any(not(test), feature = "bls"))]
    fn validate_worker_readback_response_body(
        &self,
        request: &wire::CertifiedBodyRequest,
        body_readback: &DurableCertifiedServeBodyReadbackV1,
        expected_body_store: &V2BodyStoreInstanceIdentity,
        response: &wire::CertifiedBodyResponse,
    ) -> Result<(), CertifiedServePayloadStoreError> {
        if !body_readback.matches_store_instance(expected_body_store) {
            return Err(CertifiedServePayloadStoreError::ForeignBodyStore);
        }
        let durable_body = body_readback.durable_body();
        if durable_body.context_id() != self.context.id()
            || durable_body.round() != request.round
            || durable_body.subject() != request.subject
            || response.manifest.round != request.round
            || response.manifest.subject != request.subject
            || durable_body.manifest_hash() != HashOf::new(&response.manifest)
        {
            return Err(CertifiedServePayloadStoreError::DurableBodyReceiptMismatch);
        }
        let canonical_body = body_readback.canonical_wire();
        if u64::try_from(canonical_body.len()).ok() != Some(response.manifest.payload_size_bytes)
            || Hash::new(canonical_body) != durable_body.subject().payload_hash
        {
            return Err(CertifiedServePayloadStoreError::InvalidDurableBody(
                "worker readback lost its exact manifest/body binding".to_owned(),
            ));
        }
        if canonical_body != response.body.as_slice() {
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
        let responder_index = self
            .context
            .roster
            .iter()
            .position(|entry| entry.validator == response.responder)
            .ok_or_else(|| {
                invalid_frame(
                    &self.directory,
                    "response signer is outside the frozen roster",
                )
            })?;
        let responder_index = wire::ValidatorIndex::try_from(responder_index).map_err(|_| {
            invalid_frame(
                &self.directory,
                "response signer index is not representable",
            )
        })?;
        if request
            .certificate
            .signers
            .binary_search(&responder_index)
            .is_err()
        {
            return Err(invalid_frame(
                &self.directory,
                "response signer lost certified local retention authority",
            ));
        }
        let responder = &response.responder;
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
        self.load_id_with_leaf(id).map(|(payload, _)| payload)
    }
    fn load_id_with_leaf(
        &self,
        id: CertifiedServePayloadId,
    ) -> Result<
        (
            PersistedCertifiedServePayloadV1,
            BoundCertifiedServePayloadLeaf,
        ),
        CertifiedServePayloadStoreError,
    > {
        let census = self.reload_payload_census_strict()?;
        if census.keys().copied().collect::<BTreeSet<_>>() != self.indexed {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch);
        }
        let expected = census
            .get(&id)
            .ok_or(CertifiedServePayloadStoreError::UnknownPayload)?;
        let (payload, leaf, terminal_companion) = self.load_id_with_leaf_untracked_terminal(id)?;
        if terminal_companion.as_ref() != self.terminal_companions.get(&id) {
            return Err(invalid_frame(
                &self.terminal_path_for(id),
                "exact terminal companion changed behind the retained store owner",
            ));
        }
        if &payload != expected {
            return Err(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch);
        }
        Ok((payload, leaf))
    }
    fn load_id_with_leaf_untracked_terminal(
        &self,
        id: CertifiedServePayloadId,
    ) -> Result<
        (
            PersistedCertifiedServePayloadV1,
            BoundCertifiedServePayloadLeaf,
            Option<BoundCertifiedServePayloadLeaf>,
        ),
        CertifiedServePayloadStoreError,
    > {
        let name = self.file_name_for(id);
        let path = self.directory.join(&name);
        let removal_name = self.removal_file_name_for(id);
        if self
            .bound_directory()?
            .inspect_leaf(&removal_name, self.max_entry_bytes)?
            .is_some()
        {
            return Err(invalid_frame(
                &path,
                "active payload coexists with its removal journal",
            ));
        }
        let leaf = self
            .bound_directory()?
            .inspect_leaf(&name, self.max_entry_bytes)?
            .ok_or_else(|| CertifiedServePayloadStoreError::NonRegularEntry(path.clone()))?;
        let (canonical, canonical_leaf) = self.load_leaf_with_bound(&leaf)?;
        if canonical.id() != id {
            return Err(CertifiedServePayloadStoreError::RequestHashFilenameMismatch(path));
        }
        if !matches!(
            &canonical.state,
            PersistedCertifiedServePayloadStateV1::Pending
        ) {
            return Err(invalid_frame(
                &self.path_for(id),
                "canonical payload must remain the immutable Pending frame",
            ));
        }
        let terminal_name = self.terminal_file_name_for(id);
        let Some(terminal_leaf) = self
            .bound_directory()?
            .inspect_leaf(&terminal_name, self.max_entry_bytes)?
        else {
            return Ok((canonical, canonical_leaf, None));
        };
        let terminal_path = self.directory.join(&terminal_name);
        let (terminal, terminal_leaf) = self.load_leaf_with_bound(&terminal_leaf)?;
        if terminal.id() != id {
            return Err(
                CertifiedServePayloadStoreError::RequestHashFilenameMismatch(terminal_path),
            );
        }
        if !terminal_companion_matches(&canonical, &terminal) {
            return Err(invalid_frame(
                &terminal_path,
                "terminal companion does not extend the exact canonical Pending frame",
            ));
        }
        Ok((terminal, terminal_leaf.clone(), Some(terminal_leaf)))
    }
    fn load_leaf_with_bound(
        &self,
        leaf: &BoundCertifiedServePayloadLeaf,
    ) -> Result<
        (
            PersistedCertifiedServePayloadV1,
            BoundCertifiedServePayloadLeaf,
        ),
        CertifiedServePayloadStoreError,
    > {
        let path = self.directory.join(&leaf.name);
        let bytes = self
            .bound_directory()?
            .read_leaf(leaf, self.max_entry_bytes)?;
        let payload = decode_frame(&bytes, self.max_entry_bytes, &path)?;
        self.validate_recovered_payload(&payload, &path)?;
        let exact_leaf = BoundCertifiedServePayloadLeaf {
            frame_hash: Some(Hash::new(&bytes)),
            ..leaf.clone()
        };
        Ok((payload, exact_leaf))
    }
    fn persist_payload(
        &mut self,
        payload: &PersistedCertifiedServePayloadV1,
        expected_destination: Option<&BoundCertifiedServePayloadLeaf>,
    ) -> Result<(), PersistPayloadError> {
        self.ensure_mutable()
            .map_err(PersistPayloadError::Unpublished)?;
        let path = self.path_for(payload.id());
        self.validate_recovered_payload(payload, &path)
            .map_err(PersistPayloadError::Unpublished)?;
        let (frame, _) = encode_frame(payload, self.max_entry_bytes)
            .map_err(PersistPayloadError::Unpublished)?;
        #[cfg(test)]
        let fail_before_directory_sync = std::mem::take(&mut self.fail_next_publish_directory_sync);
        #[cfg(not(test))]
        let fail_before_directory_sync = false;
        #[cfg(test)]
        let replace_terminal_canonical = if expected_destination.is_some() {
            self.replace_next_terminal_canonical_before_companion_create
                .take()
        } else {
            None
        };
        #[cfg(test)]
        let race_publication_destination = self
            .race_next_publication_destination_before_noreplace
            .take();
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            let id = payload.id();
            let canonical_name = self.file_name_for(id);
            let staging_name = self.temporary_file_name_for(id);
            let terminal_name = self.terminal_file_name_for(id);
            let removal_name = self.removal_file_name_for(id);
            let (publication_name, is_terminal) = if expected_destination.is_some() {
                if matches!(
                    &payload.state,
                    PersistedCertifiedServePayloadStateV1::Pending
                ) {
                    return Err(PersistPayloadError::Unpublished(invalid_frame(
                        &path,
                        "terminal publication cannot retain a Pending companion",
                    )));
                }
                (&terminal_name, true)
            } else {
                if !matches!(
                    &payload.state,
                    PersistedCertifiedServePayloadStateV1::Pending
                ) {
                    return Err(PersistPayloadError::Unpublished(invalid_frame(
                        &path,
                        "fresh canonical publication must be Pending",
                    )));
                }
                (&canonical_name, false)
            };
            let bound = self
                .bound_directory()
                .map_err(PersistPayloadError::Unpublished)?;
            bound
                .validate_publication_destination(&removal_name, self.max_entry_bytes, None)
                .map_err(PersistPayloadError::Unpublished)?;
            bound
                .validate_publication_destination(publication_name, self.max_entry_bytes, None)
                .map_err(PersistPayloadError::Unpublished)?;
            bound
                .validate_publication_destination(&staging_name, self.max_entry_bytes, None)
                .map_err(PersistPayloadError::Unpublished)?;
            if let Some(expected) = expected_destination {
                bound
                    .validate_publication_destination(
                        &canonical_name,
                        self.max_entry_bytes,
                        Some(expected),
                    )
                    .map_err(PersistPayloadError::Unpublished)?;
            } else {
                bound
                    .validate_publication_destination(&terminal_name, self.max_entry_bytes, None)
                    .map_err(PersistPayloadError::Unpublished)?;
            }
            let before = self
                .reload_payload_census_strict()
                .map_err(PersistPayloadError::Unpublished)?;
            if before.keys().copied().collect::<BTreeSet<_>>() != self.indexed {
                return Err(PersistPayloadError::Unpublished(
                    CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch,
                ));
            }
            // Crash/linearisation contract:
            // - before exclusive create, no state changed;
            // - after create or a partial write, `.tmp` is non-authoritative
            //   and startup moves it into a bounded, never-reused quarantine;
            // - after file fsync but before NOREPLACE, an exact terminal stage
            //   may resume because the durable canonical Pending frame already
            //   binds it, while a Pending stage is quarantined because no
            //   admission receipt was durably returned;
            // - after NOREPLACE but before directory fsync, recovery accepts
            //   whichever atomic side survived (stage, final, or absence) by
            //   the same rules and never fabricates or overwrites a final;
            // - only file fsync + NOREPLACE + directory fsync + exact re-read
            //   returns a receipt. No authoritative name is directly created,
            //   replaced, or unlinked.
            let (file, staged_leaf) =
                bound.create_synced_leaf(&staging_name, &frame, self.max_entry_bytes)?;
            let (staged_payload, exact_staged_leaf) = self
                .load_leaf_with_bound(&staged_leaf)
                .map_err(PersistPayloadError::PublishedButUnsynchronized)?;
            if &staged_payload != payload || exact_staged_leaf != staged_leaf {
                return Err(PersistPayloadError::PublishedButUnsynchronized(
                    invalid_frame(
                        &self.directory.join(&staging_name),
                        "staging changed before final publication",
                    ),
                ));
            }
            #[cfg(test)]
            if let Some(replacement) = replace_terminal_canonical
                && let Err(source) = fs::rename(replacement, &path)
            {
                return Err(PersistPayloadError::PublishedButUnsynchronized(io_error(
                    "inject competing terminal canonical replacement",
                    &path,
                    source,
                )));
            }
            if let Some(expected) = expected_destination {
                bound
                    .validate_publication_destination(
                        &canonical_name,
                        self.max_entry_bytes,
                        Some(expected),
                    )
                    .map_err(PersistPayloadError::PublishedButUnsynchronized)?;
            }
            bound
                .validate_publication_destination(&removal_name, self.max_entry_bytes, None)
                .and_then(|()| {
                    bound.validate_publication_destination(
                        publication_name,
                        self.max_entry_bytes,
                        None,
                    )
                })
                .map_err(PersistPayloadError::PublishedButUnsynchronized)?;
            if !bound
                .leaf_at_exactly_matches(&staging_name, self.max_entry_bytes, &exact_staged_leaf)
                .map_err(PersistPayloadError::PublishedButUnsynchronized)?
            {
                return Err(PersistPayloadError::PublishedButUnsynchronized(
                    CertifiedServePayloadStoreError::PublicationConflict(
                        self.directory.join(&staging_name),
                    ),
                ));
            }
            #[cfg(test)]
            if let Some(racer) = race_publication_destination
                && let Err(source) = fs::rename(racer, self.directory.join(publication_name))
            {
                return Err(PersistPayloadError::PublishedButUnsynchronized(io_error(
                    "inject competing final-name publication",
                    &self.directory.join(publication_name),
                    source,
                )));
            }
            rename_certified_serve_leaf_noreplace(
                &bound.directory,
                &staging_name,
                publication_name,
            )
            .map_err(|source| {
                PersistPayloadError::PublishedButUnsynchronized(io_error(
                    "publish staged Certified-Serve payload",
                    &self.directory.join(publication_name),
                    source,
                ))
            })?;
            let published_leaf = BoundCertifiedServePayloadLeaf {
                name: publication_name.clone(),
                ..exact_staged_leaf
            };
            let validate_publication = || {
                bound.validate_publication_destination(
                    &staging_name,
                    self.max_entry_bytes,
                    None,
                )?;
                if let Some(expected) = expected_destination {
                    bound.validate_publication_destination(
                        &canonical_name,
                        self.max_entry_bytes,
                        Some(expected),
                    )?;
                }
                bound.validate_publication_destination(
                    &removal_name,
                    self.max_entry_bytes,
                    None,
                )?;
                bound.verify_open_leaf(&file, &published_leaf)?;
                if !bound.leaf_at_exactly_matches(
                    publication_name,
                    self.max_entry_bytes,
                    &published_leaf,
                )? {
                    return Err(CertifiedServePayloadStoreError::PublicationConflict(
                        self.directory.join(publication_name),
                    ));
                }
                let (observed, _leaf, has_terminal) =
                    self.load_id_with_leaf_untracked_terminal(id)?;
                if &observed != payload || has_terminal.is_some() != is_terminal {
                    return Err(invalid_frame(
                        &self.directory.join(publication_name),
                        "published payload pair changed before authentication",
                    ));
                }
                Ok(())
            };
            validate_publication().map_err(PersistPayloadError::PublishedButUnsynchronized)?;
            if fail_before_directory_sync {
                return Err(PersistPayloadError::PublishedButUnsynchronized(io_error(
                    "synchronise directory after published file",
                    &self.directory,
                    std::io::Error::other("injected final-name directory sync failure"),
                )));
            }
            bound
                .sync()
                .map_err(PersistPayloadError::PublishedButUnsynchronized)?;
            validate_publication().map_err(PersistPayloadError::PublishedButUnsynchronized)?;
            let after = self
                .scan_payload_census(false)
                .map_err(PersistPayloadError::PublishedButUnsynchronized)?;
            let mut expected_ids = self.indexed.clone();
            expected_ids.insert(id);
            let mut expected_terminal_companions = self.terminal_companions.clone();
            if is_terminal {
                expected_terminal_companions.insert(id, published_leaf.clone());
            }
            if after.payloads.keys().copied().collect::<BTreeSet<_>>() != expected_ids
                || after.payloads.get(&id) != Some(payload)
                || after.terminal_companions != expected_terminal_companions
                || after.removed != self.removed
                || after.quarantine != self.quarantine
            {
                return Err(PersistPayloadError::PublishedButUnsynchronized(
                    CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch,
                ));
            }
            if is_terminal {
                let inserted = self.terminal_companions.insert(id, published_leaf);
                debug_assert!(
                    inserted.is_none(),
                    "new terminal companion was already tracked"
                );
            }
            Ok(())
        }
        #[cfg(not(all(unix, not(target_os = "espidf"))))]
        {
            let _ = (
                payload,
                expected_destination,
                frame,
                fail_before_directory_sync,
                #[cfg(test)]
                replace_terminal_canonical,
                #[cfg(test)]
                race_publication_destination,
            );
            Err(PersistPayloadError::Unpublished(
                CertifiedServePayloadStoreError::UnsupportedStorageBinding(self.directory.clone()),
            ))
        }
    }
    /// Inject one admission publication failure after rename and before the
    /// final directory fsync.
    #[cfg(test)]
    pub(crate) fn fail_next_publish_directory_sync_for_test(&mut self) {
        self.fail_next_publish_directory_sync = true;
    }
    #[cfg(test)]
    fn replace_next_terminal_canonical_before_companion_create_for_test(
        &mut self,
        replacement: PathBuf,
    ) {
        assert!(
            self.replace_next_terminal_canonical_before_companion_create
                .replace(replacement)
                .is_none(),
            "only one forced terminal destination race may be armed"
        );
    }
    #[cfg(test)]
    fn race_next_publication_destination_before_noreplace_for_test(&mut self, racer: PathBuf) {
        assert!(
            self.race_next_publication_destination_before_noreplace
                .replace(racer)
                .is_none(),
            "only one forced final-name race may be armed"
        );
    }
    fn path_for(&self, id: CertifiedServePayloadId) -> PathBuf {
        self.directory.join(self.file_name_for(id))
    }
    fn file_name_for(&self, id: CertifiedServePayloadId) -> OsString {
        format!("{}{}", hex::encode(id.request_hash().as_ref()), FILE_SUFFIX).into()
    }
    #[cfg(test)]
    fn temporary_path(&self, id: CertifiedServePayloadId) -> PathBuf {
        self.directory.join(self.temporary_file_name_for(id))
    }
    #[cfg(test)]
    fn quarantine_path(&self, id: CertifiedServePayloadId) -> PathBuf {
        self.quarantine_path_for_slot(id, 0)
    }
    #[cfg(test)]
    fn quarantine_path_for_slot(&self, id: CertifiedServePayloadId, slot: usize) -> PathBuf {
        let staging = self.temporary_file_name_for(id);
        self.directory.join(
            quarantine_file_name_for_stage(&staging, slot)
                .expect("a canonical staging name and bounded slot always have a quarantine name"),
        )
    }
    fn temporary_file_name_for(&self, id: CertifiedServePayloadId) -> OsString {
        format!(
            "{}{}",
            hex::encode(id.request_hash().as_ref()),
            TEMPORARY_FILE_SUFFIX
        )
        .into()
    }
    fn terminal_path_for(&self, id: CertifiedServePayloadId) -> PathBuf {
        self.directory.join(self.terminal_file_name_for(id))
    }
    fn terminal_file_name_for(&self, id: CertifiedServePayloadId) -> OsString {
        format!(
            "{}{}",
            hex::encode(id.request_hash().as_ref()),
            TERMINAL_FILE_SUFFIX
        )
        .into()
    }
    fn removal_path_for(&self, id: CertifiedServePayloadId) -> PathBuf {
        self.directory.join(self.removal_file_name_for(id))
    }
    fn removal_file_name_for(&self, id: CertifiedServePayloadId) -> OsString {
        format!(
            "{}{}",
            hex::encode(id.request_hash().as_ref()),
            REMOVAL_FILE_SUFFIX
        )
        .into()
    }
}
fn admission_receipt(
    payload: &PersistedCertifiedServePayloadV1,
    local_retainer: wire::ValidatorIndex,
) -> DurableCertifiedServeAdmissionReceipt {
    let id = payload.id();
    let certificate_hash = HashOf::new(&payload.request.certificate);
    let payload_hash = payload.payload_hash();
    DurableCertifiedServeAdmissionReceipt {
        id,
        certificate_hash,
        payload_hash,
        local_retainer,
        coordinate_binding: admission_receipt_coordinate_binding(
            id,
            certificate_hash,
            payload_hash,
            local_retainer,
        ),
    }
}
fn admission_receipt_coordinate_binding(
    id: CertifiedServePayloadId,
    certificate_hash: HashOf<wire::QuorumCertificate>,
    payload_hash: Hash,
    local_retainer: wire::ValidatorIndex,
) -> Hash {
    let mut preimage = Vec::with_capacity(
        ADMISSION_RECEIPT_BINDING_DOMAIN.len()
            + 3 * Hash::LENGTH
            + size_of::<wire::ValidatorIndex>(),
    );
    preimage.extend_from_slice(ADMISSION_RECEIPT_BINDING_DOMAIN);
    preimage.extend_from_slice(id.request_hash().as_ref());
    preimage.extend_from_slice(certificate_hash.as_ref());
    preimage.extend_from_slice(payload_hash.as_ref());
    preimage.extend_from_slice(&local_retainer.to_le_bytes());
    Hash::new(preimage)
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
fn quarantine_file_name_for_stage(stage: &OsStr, slot: usize) -> Option<OsString> {
    if slot >= MAX_QUARANTINED_STAGES_PER_HEIGHT {
        return None;
    }
    let stage = stage.to_str()?;
    let hash = stage.strip_suffix(TEMPORARY_FILE_SUFFIX)?;
    has_canonical_hash_name(stage, TEMPORARY_FILE_SUFFIX)
        .then(|| format!("{hash}{QUARANTINE_FILE_SUFFIX}.{slot:02}").into())
}
fn quarantine_slot_from_file_name(name: &str) -> Option<usize> {
    let (base, slot) = name.rsplit_once('.')?;
    if slot.len() != 2 || !slot.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let slot = slot.parse::<usize>().ok()?;
    (slot < MAX_QUARANTINED_STAGES_PER_HEIGHT
        && has_canonical_hash_name(base, QUARANTINE_FILE_SUFFIX))
    .then_some(slot)
}
fn terminal_companion_matches(
    canonical: &PersistedCertifiedServePayloadV1,
    terminal: &PersistedCertifiedServePayloadV1,
) -> bool {
    if !matches!(
        &canonical.state,
        PersistedCertifiedServePayloadStateV1::Pending
    ) || matches!(
        &terminal.state,
        PersistedCertifiedServePayloadStateV1::Pending
    ) {
        return false;
    }
    let mut expected_pending = terminal.clone();
    expected_pending.state = PersistedCertifiedServePayloadStateV1::Pending;
    &expected_pending == canonical
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
#[cfg(test)]
fn sync_directory(directory: &Path) -> Result<(), CertifiedServePayloadStoreError> {
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|source| io_error("synchronise directory", directory, source))
}
#[cfg(test)]
fn ensure_durable_directory(directory: &Path) -> Result<(), CertifiedServePayloadStoreError> {
    ensure_durable_directory_with(directory, &mut sync_directory)
}
#[cfg(test)]
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
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => return Err(io_error("inspect directory", directory, source)),
    }
    ensure_durable_directory_with(parent, sync)?;
    match fs::create_dir(directory) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
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
    use super::*;
    #[cfg(feature = "bls")]
    use crate::sumeragi::v2_chunks::encode_payload;
    use crate::sumeragi::v2_transport::authenticate_certified_body_request;
    #[cfg(feature = "bls")]
    use iroha_crypto::SignatureOf;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    #[cfg(feature = "bls")]
    use iroha_data_model::block::{BlockSignature, SignedBlock};
    use iroha_data_model::{block::BlockHeader, peer::PeerId};
    #[cfg(feature = "bls")]
    use std::num::NonZeroU64;
    use tempfile::TempDir;

    #[cfg(target_os = "macos")]
    fn add_macos_read_acl(path: &Path) {
        let output = std::process::Command::new("/bin/chmod")
            .arg("+a")
            .arg("everyone allow read")
            .arg(path)
            .output()
            .expect("invoke macOS chmod for extended-ACL regression");
        assert!(
            output.status.success(),
            "macOS chmod must install the test ACL: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

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
        let network_id =
            crate::sumeragi::synthetic_network_id("certified-serve-payload-store-test");
        let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
            crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(network_id, 0, &roster);
        let context = wire::HeightContext {
            network_id,
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
            kagemusha_mint_finality_epoch_id,
            kagemusha_mint_finality_epoch_roster,
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

    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn production_open_consumes_the_exact_kura_directory_authority() {
        let first = crate::kura::Kura::blank_kura_for_testing();
        let second = crate::kura::Kura::blank_kura_for_testing();
        let deferred = crate::kura::Kura::blank_kura_for_testing();
        let (context, _) = context_and_keys();
        let deferred_lifecycle_root = deferred.sumeragi_v2_storage_root().join("lifecycle-v1");
        let mut invalid_context = context.clone();
        invalid_context.height = 0;
        assert!(
            deferred
                .mint_v2_certified_serve_payload_directory_authority(&invalid_context)
                .is_err()
        );
        assert!(
            !deferred_lifecycle_root.exists(),
            "invalid geometry must fail before Kura materializes lifecycle ancestry"
        );
        let foreign = first
            .mint_v2_certified_serve_payload_directory_authority(&context)
            .expect("mint first-Kura payload authority");
        assert!(matches!(
            CertifiedServePayloadStoreV1::open_with_kura_authority(
                second.as_ref(),
                foreign,
                &context,
            ),
            Err(CertifiedServePayloadStoreError::StorageBinding {
                reason: "payload-store authority belongs to another Kura instance",
                ..
            })
        ));

        let authority = first
            .mint_v2_certified_serve_payload_directory_authority(&context)
            .expect("mint exact payload authority");
        let lifecycle_root = first
            .sumeragi_v2_storage_root()
            .join("lifecycle-v1")
            .join(hex::encode(context.id().0.as_ref()));
        let (store, recovery) = CertifiedServePayloadStoreV1::open_with_kura_authority(
            first.as_ref(),
            authority,
            &context,
        )
        .expect("open exact Kura-bound payload store");
        assert!(recovery.payloads.is_empty());
        assert!(store.matches_lifecycle_storage_root(&lifecycle_root, &context));

        let deferred_authority = deferred
            .mint_v2_certified_serve_payload_directory_authority(&context)
            .expect("mint deferred Kura payload authority");
        let (deferred_store, deferred_recovery) =
            CertifiedServePayloadStoreV1::open_with_kura_authority(
                deferred.as_ref(),
                deferred_authority,
                &context,
            )
            .expect("open the deferred Kura target");
        assert!(deferred_recovery.payloads.is_empty());
        assert!(deferred_store.matches_lifecycle_storage_root(
            &deferred_lifecycle_root.join(hex::encode(context.id().0.as_ref())),
            &context,
        ));
    }

    #[test]
    fn emergency_fast_payload_store_skips_inventory_and_rejects_retirement() {
        let root = TempDir::new().expect("temporary emergency Serve payload store");
        let (context, _) = context_and_keys();
        let expected_directory = root.path().join(STORE_DIRECTORY);
        let (mut store, recovery) =
            CertifiedServePayloadStoreV1::open_emergency_fast_read_only(root.path(), &context)
                .expect("open inert emergency Serve payload store");
        assert!(recovery.is_empty());
        assert!(
            !expected_directory.exists(),
            "emergency open must not create or inventory the payload directory"
        );

        fs::create_dir(&expected_directory).expect("create ignored payload directory");
        let sentinel_path = expected_directory.join("unexpected");
        let sentinel = b"untouched Strict-recovery Serve payload";
        fs::write(&sentinel_path, sentinel).expect("write ignored payload sentinel");
        let authenticated = AuthenticatedCertifiedServePayloadRecoveryCut {
            context_id: context.id(),
            height: context.height,
            payloads: BTreeMap::new(),
        };
        store
            .validate_authenticated_cut(&authenticated)
            .expect("emergency validation stays inert instead of inventorying the sentinel");
        assert!(matches!(
            store.retire_authenticated_cut(authenticated, &BTreeSet::new()),
            Err(CertifiedServeTerminalPersistenceError::StoreInvariant(
                CertifiedServePayloadStoreError::EmergencyFastReadOnly
            ))
        ));
        assert_eq!(
            fs::read(&sentinel_path).expect("reread ignored payload sentinel"),
            sentinel,
            "emergency retirement rejection must not mutate retained payload bytes"
        );
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
        let network_id =
            crate::sumeragi::synthetic_network_id("certified-serve-payload-recovery-test");
        let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
            crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(network_id, 0, &roster);
        let context = wire::HeightContext {
            network_id,
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
            kagemusha_mint_finality_epoch_id,
            kagemusha_mint_finality_epoch_roster,
            nexus_amx_context_hash: Hash::new(b"Serve payload recovery AMX context"),
            execution_policy_hash: Hash::new(b"Serve payload recovery execution policy"),
            da_layout: wire::recommended_data_availability_layout(),
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
        let execution_commitment =
            wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
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
        let responder_index = usize::try_from(responder).expect("fixture responder index");
        let mut response = wire::CertifiedBodyResponse {
            request_hash: request.request_hash(),
            manifest,
            body,
            responder: PeerId::new(keys[responder_index].public_key().clone()),
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            keys[responder_index].private_key(),
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
                execution_commitment:
                    wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
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
            responder: PeerId::new(key.public_key().clone()),
            signature: Vec::new(),
        };
        response.signature = Signature::new(key.private_key(), &response.signature_preimage())
            .payload()
            .to_vec();
        (authenticated, response)
    }
    #[test]
    fn payload_store_instance_identity_distinguishes_same_path_reopen() {
        let temporary = TempDir::new().expect("temporary identity payload store");
        let (context, _) = context_and_keys();
        let (store, _) = CertifiedServePayloadStoreV1::open(temporary.path(), &context)
            .expect("open first payload-store instance");
        let first = store.instance_identity();
        assert!(first.same_instance(&store.instance_identity()));
        assert!(matches!(
            CertifiedServePayloadStoreV1::open(temporary.path(), &context),
            Err(CertifiedServePayloadStoreError::Io {
                operation: "lock store directory",
                ..
            })
        ));
        drop(store);
        let (reopened, _) = CertifiedServePayloadStoreV1::open(temporary.path(), &context)
            .expect("reopen the released payload-store path");
        assert!(
            !first.same_instance(&reopened.instance_identity()),
            "path and context equality cannot substitute for exact Serve-store ownership"
        );
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
        let canonical_path = store.path_for(pending.id());
        let canonical_pending = fs::read(&canonical_path).expect("read canonical Pending frame");
        assert!(pending.exactly_matches_pending(&request));
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
        let terminal_path = store.terminal_path_for(completed.id());
        assert_eq!(
            fs::read(&canonical_path).expect("reread immutable canonical Pending frame"),
            canonical_pending
        );
        assert!(
            terminal_path.exists(),
            "terminal companion must be retained"
        );
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
        assert_eq!(completed_ref.responder(), 0);
        assert_eq!(completed_ref.signature(), response.signature);
        assert_eq!(
            fs::read(&canonical_path).expect("reread recovered canonical Pending frame"),
            canonical_pending
        );
        assert!(terminal_path.exists());
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn live_store_rejects_a_valid_terminal_companion_replacement() {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = TempDir::new().expect("temporary terminal-replacement directory");
        let (context, keys) = context_and_keys();
        let (request, _) =
            request_and_response(&context, &keys[0], 0, b"terminal-replace".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let pending = store.persist_pending(&request).expect("persist pending");
        let _ = store
            .persist_negative(
                pending.id(),
                CertifiedServePayloadNegativeOutcome::Rejected(17),
            )
            .expect("persist original terminal companion");

        let mut replacement = store.load_id(pending.id()).expect("load terminal payload");
        replacement.state = PersistedCertifiedServePayloadStateV1::Negative {
            outcome: CertifiedServePayloadNegativeOutcome::Rejected(18),
        };
        let (replacement_frame, _) = encode_frame(&replacement, store.max_entry_bytes)
            .expect("encode structurally valid replacement terminal");
        let replacement_path = temporary.path().join("replacement-terminal");
        fs::write(&replacement_path, &replacement_frame).expect("write replacement terminal");
        fs::set_permissions(
            &replacement_path,
            fs::Permissions::from_mode(PRIVATE_LEAF_MODE),
        )
        .expect("make replacement terminal private");
        File::open(&replacement_path)
            .and_then(|file| file.sync_all())
            .expect("synchronise replacement terminal");
        fs::rename(&replacement_path, store.terminal_path_for(pending.id()))
            .expect("atomically replace terminal companion");

        assert!(matches!(
            store.load_id(pending.id()),
            Err(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch)
        ));
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn terminal_publication_rejects_same_inode_same_length_content_drift() {
        use std::os::unix::fs::MetadataExt as _;

        let temporary = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"content-cas".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let pending = store.persist_pending(&request).expect("persist pending");
        let (mut payload, incumbent) = store
            .load_id_with_leaf(pending.id())
            .expect("load exact pending leaf");
        let path = store.path_for(pending.id());
        let original_metadata = fs::metadata(&path).expect("inspect pending leaf");
        let mut drifted = fs::read(&path).expect("read pending leaf");
        let last = drifted
            .last_mut()
            .expect("framed pending leaf is non-empty");
        *last ^= 0x01;
        fs::write(&path, &drifted).expect("rewrite pending leaf in place");
        File::open(&path)
            .and_then(|file| file.sync_all())
            .expect("synchronise drifted pending leaf");
        let drifted_metadata = fs::metadata(&path).expect("reinspect drifted pending leaf");
        assert_eq!(drifted_metadata.dev(), original_metadata.dev());
        assert_eq!(drifted_metadata.ino(), original_metadata.ino());
        assert_eq!(drifted_metadata.len(), original_metadata.len());

        payload.state = PersistedCertifiedServePayloadStateV1::Negative {
            outcome: CertifiedServePayloadNegativeOutcome::Rejected(9),
        };
        assert!(matches!(
            store.persist_payload(&payload, Some(&incumbent)),
            Err(PersistPayloadError::Unpublished(
                CertifiedServePayloadStoreError::PublicationConflict(conflict)
            )) if conflict == path
        ));
        assert_eq!(
            fs::read(&path).expect("reread drifted pending leaf"),
            drifted
        );
        assert!(!store.terminal_path_for(pending.id()).exists());
    }
    #[cfg(all(
        unix,
        not(target_os = "espidf"),
        any(target_vendor = "apple", target_os = "linux", target_os = "android")
    ))]
    #[test]
    fn terminal_companion_race_preserves_the_replacement_and_fails_closed() {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = TempDir::new().expect("temporary terminal-companion race directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"race-cas".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let pending = store.persist_pending(&request).expect("persist pending");
        let destination = store.path_for(pending.id());
        let replacement = temporary.path().join("racing-canonical-replacement");
        let replacement_bytes = b"non-cooperating replacement must survive";
        fs::write(&replacement, replacement_bytes).expect("write racing replacement");
        fs::set_permissions(&replacement, fs::Permissions::from_mode(PRIVATE_LEAF_MODE))
            .expect("set racing replacement private");
        File::open(&replacement)
            .and_then(|file| file.sync_all())
            .expect("synchronise racing replacement");
        store.replace_next_terminal_canonical_before_companion_create_for_test(replacement.clone());

        assert!(matches!(
            store.persist_negative(
                pending.id(),
                CertifiedServePayloadNegativeOutcome::Rejected(9),
            ),
            Err(CertifiedServePayloadStoreError::PublicationConflict(path)) if path == destination
        ));
        assert_eq!(
            fs::read(&destination).expect("read racing canonical replacement"),
            replacement_bytes
        );
        assert!(
            !replacement.exists(),
            "the injected rename must consume its source"
        );
        let terminal_path = store.terminal_path_for(pending.id());
        let staging_path = store.temporary_path(pending.id());
        assert!(
            !terminal_path.exists(),
            "the terminal destination must remain absent after the canonical race"
        );
        let staged = fs::read(&staging_path).expect("read retained terminal staging");
        drop(store);
        assert!(CertifiedServePayloadStoreV1::open(temporary.path(), &context).is_err());
        assert_eq!(
            fs::read(&destination).expect("reread racer"),
            replacement_bytes
        );
        assert_eq!(
            fs::read(&staging_path).expect("reread retained terminal staging"),
            staged
        );
    }

    #[cfg(all(
        unix,
        not(target_os = "espidf"),
        any(target_vendor = "apple", target_os = "linux", target_os = "android")
    ))]
    #[test]
    fn terminal_noreplace_race_never_overwrites_the_competing_final() {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = TempDir::new().expect("temporary terminal final-name race directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"noreplac".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let pending = store
            .persist_pending(&request)
            .expect("persist Pending frame");
        let canonical_path = store.path_for(pending.id());
        let canonical = fs::read(&canonical_path).expect("read canonical Pending frame");
        let terminal_path = store.terminal_path_for(pending.id());
        let staging_path = store.temporary_path(pending.id());
        let racer = temporary.path().join("racing-terminal-final");
        let racer_bytes = b"same-uid competing terminal must survive";
        fs::write(&racer, racer_bytes).expect("write competing terminal final");
        fs::set_permissions(&racer, fs::Permissions::from_mode(PRIVATE_LEAF_MODE))
            .expect("set competing terminal private");
        File::open(&racer)
            .and_then(|file| file.sync_all())
            .expect("synchronise competing terminal");
        store.race_next_publication_destination_before_noreplace_for_test(racer.clone());

        assert!(matches!(
            store.persist_negative(
                pending.id(),
                CertifiedServePayloadNegativeOutcome::Rejected(29),
            ),
            Err(CertifiedServePayloadStoreError::Io { path, .. }) if path == terminal_path
        ));
        assert!(!racer.exists());
        assert_eq!(
            fs::read(&canonical_path).expect("reread canonical Pending frame"),
            canonical
        );
        assert_eq!(
            fs::read(&terminal_path).expect("read competing terminal final"),
            racer_bytes
        );
        let staged = fs::read(&staging_path).expect("read retained terminal staging");
        drop(store);
        assert!(CertifiedServePayloadStoreV1::open(temporary.path(), &context).is_err());
        assert_eq!(
            fs::read(&terminal_path).expect("reread competing terminal final"),
            racer_bytes
        );
        assert_eq!(
            fs::read(&staging_path).expect("reread retained terminal staging"),
            staged
        );
    }

    #[cfg(all(
        unix,
        not(target_os = "espidf"),
        any(target_vendor = "apple", target_os = "linux", target_os = "android")
    ))]
    #[test]
    fn terminal_publication_rejects_a_preexisting_companion_before_mutation() {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = TempDir::new().expect("temporary preexisting-companion directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"armed-cas".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let pending = store.persist_pending(&request).expect("persist pending");
        let destination = store.path_for(pending.id());
        let terminal = store.terminal_path_for(pending.id());
        let staging = store.temporary_path(pending.id());
        let incumbent = fs::read(&destination).expect("read incumbent Pending frame");
        let stale_staging = b"preexisting staging must remain inert after a final conflict";
        fs::write(&staging, stale_staging).expect("write preexisting staging artifact");
        fs::set_permissions(&staging, fs::Permissions::from_mode(PRIVATE_LEAF_MODE))
            .expect("set preexisting staging private");
        fs::write(&terminal, &incumbent).expect("write preexisting terminal artifact");
        fs::set_permissions(&terminal, fs::Permissions::from_mode(PRIVATE_LEAF_MODE))
            .expect("set preexisting terminal private");
        File::open(&terminal)
            .and_then(|file| file.sync_all())
            .expect("synchronise preexisting terminal");

        assert!(matches!(
            store.persist_negative(
                pending.id(),
                CertifiedServePayloadNegativeOutcome::Rejected(10),
            ),
            Err(CertifiedServePayloadStoreError::PublicationConflict(path)) if path == terminal
        ));
        assert_eq!(
            fs::read(&destination).expect("reread untouched incumbent"),
            incumbent
        );
        assert_eq!(
            fs::read(&terminal).expect("reread preserved terminal artifact"),
            incumbent
        );
        assert_eq!(
            fs::read(&staging).expect("reread preserved staging artifact"),
            stale_staging
        );
    }

    #[cfg(all(
        unix,
        not(target_os = "espidf"),
        any(target_vendor = "apple", target_os = "linux", target_os = "android")
    ))]
    #[test]
    fn restart_accepts_an_authenticated_terminal_companion_pair() {
        let temporary = TempDir::new().expect("temporary terminal-companion directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"crash-cas".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let pending = store.persist_pending(&request).expect("persist pending");
        let outcome = CertifiedServePayloadNegativeOutcome::Rejected(11);
        let _ = store
            .persist_negative(pending.id(), outcome)
            .expect("persist terminal companion");
        let canonical_path = store.path_for(pending.id());
        let terminal_path = store.terminal_path_for(pending.id());
        let canonical = fs::read(&canonical_path).expect("read canonical Pending frame");
        let terminal = fs::read(&terminal_path).expect("read terminal companion");
        drop(store);

        let (_store, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), &context)
            .expect("authenticate terminal companion pair");
        assert!(matches!(
            recovery.get(pending.id()).expect("recover terminal pair").state(),
            RecoveredCertifiedServePayloadState::Negative(recovered) if recovered == outcome
        ));
        assert_eq!(
            fs::read(&canonical_path).expect("reread canonical Pending frame"),
            canonical
        );
        assert_eq!(
            fs::read(&terminal_path).expect("reread terminal companion"),
            terminal
        );
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn terminal_noreplace_before_directory_sync_is_recovered_as_the_exact_pair() {
        let temporary = TempDir::new().expect("temporary terminal durability directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"dirsync!".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let pending = store
            .persist_pending(&request)
            .expect("persist Pending frame");
        let outcome = CertifiedServePayloadNegativeOutcome::Rejected(12);
        store.fail_next_publish_directory_sync_for_test();
        assert!(matches!(
            store.persist_negative(pending.id(), outcome),
            Err(CertifiedServePayloadStoreError::Io { .. })
        ));
        let canonical_path = store.path_for(pending.id());
        let staging_path = store.temporary_path(pending.id());
        let terminal_path = store.terminal_path_for(pending.id());
        assert!(canonical_path.exists());
        assert!(!staging_path.exists());
        assert!(terminal_path.exists());
        drop(store);

        let (_store, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), &context)
            .expect("recover terminal final selected before directory fsync");
        assert!(matches!(
            recovery.get(pending.id()).expect("recover terminal pair").state(),
            RecoveredCertifiedServePayloadState::Negative(recovered) if recovered == outcome
        ));
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn startup_census_rejects_a_non_private_payload_leaf() {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = TempDir::new().expect("temporary permissive-mode directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"mode-open".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let pending = store.persist_pending(&request).expect("persist pending");
        let path = store.path_for(pending.id());
        drop(store);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .expect("make retained leaf non-private");

        assert!(matches!(
            CertifiedServePayloadStoreV1::open(temporary.path(), &context),
            Err(CertifiedServePayloadStoreError::NonRegularEntry(rejected)) if rejected == path
        ));
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn live_store_rejects_payload_leaf_mode_drift() {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = TempDir::new().expect("temporary live mode-drift directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"mode-live".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let pending = store.persist_pending(&request).expect("persist pending");
        let path = store.path_for(pending.id());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o660))
            .expect("make live leaf group-writable");

        assert!(matches!(
            store.persist_negative(
                pending.id(),
                CertifiedServePayloadNegativeOutcome::Rejected(9),
            ),
            Err(CertifiedServePayloadStoreError::NonRegularEntry(rejected)) if rejected == path
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn startup_rejects_a_private_mode_leaf_with_an_extended_acl() {
        use std::os::unix::fs::MetadataExt as _;

        let temporary = TempDir::new().expect("temporary ACL payload directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"acl-leaf".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let pending = store.persist_pending(&request).expect("persist pending");
        let path = store.path_for(pending.id());
        drop(store);
        add_macos_read_acl(&path);
        assert_eq!(
            fs::metadata(&path).expect("inspect ACL leaf").mode() & 0o7777,
            PRIVATE_LEAF_MODE,
            "the extended ACL must not be visible through traditional mode bits"
        );

        assert!(matches!(
            CertifiedServePayloadStoreV1::open(temporary.path(), &context),
            Err(CertifiedServePayloadStoreError::NonRegularEntry(rejected)) if rejected == path
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn startup_rejects_a_store_directory_with_an_extended_acl() {
        let temporary = TempDir::new().expect("temporary ACL store directory");
        let (context, _) = context_and_keys();
        let (store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let directory = store.directory.clone();
        drop(store);
        add_macos_read_acl(&directory);

        assert!(matches!(
            CertifiedServePayloadStoreV1::open(temporary.path(), &context),
            Err(CertifiedServePayloadStoreError::InvalidStoreDirectory(rejected))
                if rejected == directory
        ));
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
        nonretaining.responder = PeerId::new(keys[3].public_key().clone());
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
    fn only_the_call_that_created_pending_owns_preledger_abort_authority() {
        let temporary = TempDir::new().expect("temporary fresh Pending authority store");
        let (verified, keys) = verified_bls_context_and_keys();
        let request = bls_request(&verified, &keys, true);
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("open fresh Pending authority store");
        let fresh = store
            .retain_for_admission_with_verified_retention(&verified, &keys[0], &request)
            .expect("create exact Pending frame");
        assert!(fresh.can_abort_fresh_pending());
        assert_eq!(
            fresh.state(),
            DurableCertifiedServeAdmissionStateV1::Pending
        );
        let receipt = fresh.receipt();
        drop(fresh);
        let repeated = store
            .retain_for_admission_with_verified_retention(&verified, &keys[0], &request)
            .expect("reuse exact Pending frame");
        assert!(!repeated.can_abort_fresh_pending());
        assert_eq!(repeated.receipt(), receipt);
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
        let _ = store
            .persist_negative(pending.id(), outcome)
            .expect("persist typed negative");
        let live_retirement = store
            .authenticate_current_for_lifecycle_retirement(
                crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleServeRetirementAuthenticationPermitV1::for_test(),
                &verified,
                &keys[0],
            )
            .expect("refresh the exact live retirement-only payload census");
        assert!(matches!(
            live_retirement
                .get(pending.id())
                .expect("live retirement cut contains the new terminal")
                .state(),
            AuthenticatedRecoveredCertifiedServePayloadState::Negative(recovered)
                if *recovered == outcome
        ));
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
        assert_eq!(recovered.local_retainer(), 0);
        assert!(recovered.exactly_matches_persisted_payload());
        assert_ne!(recovered.payload_hash(), pending.payload_hash());
        assert!(matches!(
            recovered.state(),
            AuthenticatedRecoveredCertifiedServePayloadState::Negative(recovered)
                if *recovered == outcome
        ));
    }
    #[cfg(feature = "bls")]
    #[test]
    fn retirement_refreshes_retained_pending_and_prunes_only_pending_orphans() {
        let temporary = TempDir::new().expect("temporary retirement payload store");
        let (verified, keys) = verified_bls_context_and_keys();
        let retained_request = bls_request(&verified, &keys, true);
        let orphan_round = wire::ConsensusRound {
            view: 1,
            ..retained_request.request().round
        };
        let orphan_subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"orphan CompleteTip Serve request",
            )),
            payload_hash: Hash::new(b"orphan CompleteTip Serve body"),
        };
        let orphan_request =
            bls_request_for_subject(&verified, &keys, true, orphan_round, orphan_subject);
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("open retirement payload store");
        let retained = store
            .persist_pending_with_verified_retention(&verified, &keys[0], &retained_request)
            .expect("persist retained pending Serve");
        let orphan = store
            .persist_pending_with_verified_retention(&verified, &keys[0], &orphan_request)
            .expect("persist orphan pending Serve");
        drop(store);
        let body_store = V2BodyStore::open(temporary.path(), verified.context().clone())
            .expect("open empty exact body store");
        let (mut store, recovery) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("reopen retirement payload store");
        let authenticated = recovery
            .authenticate(&verified, &keys[0], &body_store)
            .expect("authenticate both pending payloads");
        let retained_ids = BTreeSet::from([retained.id()]);
        let retired = store
            .retire_authenticated_cut(authenticated, &retained_ids)
            .expect("prune the orphan and cancel the retained payload");
        assert_eq!(retired.len(), 1);
        assert!(matches!(
            retired
                .get(retained.id())
                .expect("retained payload remains authenticated")
                .state(),
            AuthenticatedRecoveredCertifiedServePayloadState::Negative(
                CertifiedServePayloadNegativeOutcome::Cancelled
            )
        ));
        assert!(retired.get(orphan.id()).is_none());
        store
            .validate_authenticated_cut(&retired)
            .expect("refreshed cut exactly covers the post-retirement store");
    }
    #[cfg(feature = "bls")]
    #[test]
    fn authenticated_cut_has_one_exclusive_store_owner() {
        let temporary = TempDir::new().expect("temporary exact-census payload store");
        let (verified, keys) = verified_bls_context_and_keys();
        let (first_owner, recovery) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("open first exact-census payload owner");
        let body_store = V2BodyStore::open(temporary.path(), verified.context().clone())
            .expect("open exact body store");
        let authenticated = recovery
            .authenticate(&verified, &keys[0], &body_store)
            .expect("authenticate the original empty payload census");
        assert!(authenticated.is_empty());
        assert!(matches!(
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context()),
            Err(CertifiedServePayloadStoreError::Io {
                operation: "lock store directory",
                ..
            })
        ));
        first_owner
            .validate_authenticated_cut(&authenticated)
            .expect("exclusive ownership preserves the authenticated census");
    }
    #[cfg(unix)]
    #[test]
    fn authenticated_cut_rejects_store_directory_inode_replacement() {
        let temporary = TempDir::new().expect("temporary inode-drift payload store");
        let (context, _) = context_and_keys();
        let (owner, authenticated) = CertifiedServePayloadStoreV1::open_lifecycle_fixture_for_test(
            temporary.path(),
            &context,
        )
        .expect("open retained payload directory");
        let canonical = temporary.path().join(STORE_DIRECTORY);
        let displaced = temporary.path().join("displaced-payload-store");
        fs::rename(&canonical, &displaced).expect("displace retained payload directory");
        fs::create_dir(&canonical).expect("replace retained payload directory inode");
        assert!(matches!(
            owner.validate_authenticated_cut(&authenticated),
            Err(CertifiedServePayloadStoreError::InvalidStoreDirectory(path)) if path == canonical
        ));
    }
    #[cfg(unix)]
    #[test]
    fn opening_rejects_a_store_directory_symlink() {
        let root = TempDir::new().expect("symlinked payload-store root");
        let target = TempDir::new().expect("payload-store symlink target");
        let (context, _) = context_and_keys();
        let directory = root.path().join(STORE_DIRECTORY);
        std::os::unix::fs::symlink(target.path(), &directory)
            .expect("substitute payload-store directory symlink");
        assert!(matches!(
            CertifiedServePayloadStoreV1::open(root.path(), &context),
            Err(CertifiedServePayloadStoreError::NonRegularEntry(path)) if path == directory
        ));
    }
    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn opening_rejects_symlink_hardlink_and_fifo_payload_leaves() {
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"unsafe-leaf".to_vec());
        let id = CertifiedServePayloadId::from_request(request.request());

        let symlink_root = TempDir::new().expect("symlink payload root");
        let symlink_directory = symlink_root.path().join(STORE_DIRECTORY);
        ensure_durable_directory(&symlink_directory).expect("create symlink payload directory");
        let symlink_sentinel = symlink_root.path().join("symlink-sentinel");
        fs::write(&symlink_sentinel, b"symlink sentinel").expect("write symlink sentinel");
        std::os::unix::fs::symlink(
            &symlink_sentinel,
            symlink_directory.join(format!(
                "{}{}",
                hex::encode(id.request_hash().as_ref()),
                FILE_SUFFIX
            )),
        )
        .expect("substitute payload symlink");
        assert!(matches!(
            CertifiedServePayloadStoreV1::open(symlink_root.path(), &context),
            Err(CertifiedServePayloadStoreError::NonRegularEntry(_))
        ));
        assert_eq!(
            fs::read(&symlink_sentinel).expect("reread symlink sentinel"),
            b"symlink sentinel"
        );

        let hardlink_root = TempDir::new().expect("hardlink payload root");
        let hardlink_directory = hardlink_root.path().join(STORE_DIRECTORY);
        ensure_durable_directory(&hardlink_directory).expect("create hardlink payload directory");
        let hardlink_sentinel = hardlink_root.path().join("hardlink-sentinel");
        fs::write(&hardlink_sentinel, b"hardlink sentinel").expect("write hardlink sentinel");
        fs::hard_link(
            &hardlink_sentinel,
            hardlink_directory.join(format!(
                "{}{}",
                hex::encode(id.request_hash().as_ref()),
                TEMPORARY_FILE_SUFFIX
            )),
        )
        .expect("substitute payload temporary hardlink");
        assert!(matches!(
            CertifiedServePayloadStoreV1::open(hardlink_root.path(), &context),
            Err(CertifiedServePayloadStoreError::NonRegularEntry(_))
        ));
        assert_eq!(
            fs::read(&hardlink_sentinel).expect("reread hardlink sentinel"),
            b"hardlink sentinel"
        );

        let fifo_root = TempDir::new().expect("FIFO payload root");
        let fifo_directory = fifo_root.path().join(STORE_DIRECTORY);
        ensure_durable_directory(&fifo_directory).expect("create FIFO payload directory");
        let fifo = fifo_directory.join(format!(
            "{}{}",
            hex::encode(id.request_hash().as_ref()),
            FILE_SUFFIX
        ));
        let status = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("invoke mkfifo for certified payload regression");
        assert!(status.success(), "mkfifo must create the payload fixture");
        assert!(matches!(
            CertifiedServePayloadStoreV1::open(fifo_root.path(), &context),
            Err(CertifiedServePayloadStoreError::NonRegularEntry(path)) if path == fifo
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
            payload_store.persist_completed_with_exact_body(
                &request,
                &durable_body,
                &foreign_body_store,
                &response,
            ),
            Err(CertifiedServeTerminalPersistenceError::StoreInvariant(
                CertifiedServePayloadStoreError::ForeignBodyStore
            ))
        ));
        assert!(matches!(
            payload_store.persist_completed_with_exact_body(
                &request,
                &other_durable_body,
                &body_store,
                &response,
            ),
            Err(CertifiedServeTerminalPersistenceError::InputRejected(
                CertifiedServePayloadStoreError::DurableBodyReceiptMismatch
            ))
        ));
        let mut changed_manifest = manifest.clone();
        changed_manifest.chunk_root = Hash::new(b"changed response manifest root");
        let response_with_changed_manifest =
            signed_certified_response(&request, changed_manifest, body.clone(), 0, &keys);
        assert!(matches!(
            payload_store.persist_completed_with_exact_body(
                &request,
                &durable_body,
                &body_store,
                &response_with_changed_manifest,
            ),
            Err(CertifiedServeTerminalPersistenceError::InputRejected(
                CertifiedServePayloadStoreError::DurableBodyReceiptMismatch
            ))
        ));
        let mut changed_body = body;
        changed_body[0] ^= 0x80;
        let response_with_changed_body =
            signed_certified_response(&request, manifest, changed_body, 0, &keys);
        assert!(matches!(
            payload_store.persist_completed_with_exact_body(
                &request,
                &durable_body,
                &body_store,
                &response_with_changed_body,
            ),
            Err(CertifiedServeTerminalPersistenceError::InputRejected(
                CertifiedServePayloadStoreError::DurableResponseBodyMismatch
            ))
        ));
        let completed = payload_store
            .persist_completed_with_exact_body(&request, &durable_body, &body_store, &response)
            .expect("persist receipt-backed completed response");
        assert_eq!(completed.id(), pending.id());
        assert_eq!(completed.response_hash(), HashOf::new(&response));
        assert_eq!(
            payload_store
                .persist_completed_with_exact_body(&request, &durable_body, &body_store, &response,)
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
        let _ = payload_store
            .persist_completed_with_exact_body(&request, &durable_body, &body_store, &response)
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
        assert_eq!(recovered.local_retainer(), 0);
        assert!(recovered.exactly_matches_persisted_payload());
        assert_eq!(completed.response_hash(), HashOf::new(&response));
    }
    #[cfg(feature = "bls")]
    #[test]
    fn complete_tip_retirement_authenticates_completed_metadata_after_body_cleanup() {
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
        let response = signed_certified_response(&request, manifest, body, 1, &keys);
        let _ = payload_store
            .persist_completed_with_exact_body(&request, &durable_body, &body_store, &response)
            .expect("persist completed response");
        drop(payload_store);
        drop(body_store);
        let body_directory = temporary
            .path()
            .join(hex::encode(verified.context().id().0.as_ref()));
        fs::remove_dir_all(body_directory).expect("simulate normal post-finality body cleanup");
        let (_payload_store, recovery) =
            CertifiedServePayloadStoreV1::open(temporary.path(), verified.context())
                .expect("reopen payload store after body cleanup");
        let authenticated = recovery
            .authenticate_for_complete_tip_retirement(&verified, &keys[0])
            .expect("retirement authenticates signed terminal metadata without body bytes");
        let completed = authenticated
            .get(pending.id())
            .expect("completed payload remains in retirement census");
        let AuthenticatedRecoveredCertifiedServePayloadState::Completed(completed) =
            completed.state()
        else {
            panic!("completed payload must remain terminal");
        };
        assert_eq!(completed.response_hash(), HashOf::new(&response));
        assert!(
            authenticated
                .get(pending.id())
                .expect("completed payload remains present")
                .exactly_matches_persisted_payload()
        );
    }
    #[cfg(feature = "bls")]
    #[test]
    fn retirement_rejects_completed_metadata_from_a_noncertified_responder() {
        let (verified, keys) = verified_bls_context_and_keys();
        let (body, manifest) = canonical_body_and_manifest(verified.context(), &keys, 0);
        let request =
            bls_request_for_subject(&verified, &keys, true, manifest.round, manifest.subject);
        let responder = 3;
        assert!(
            request
                .request()
                .certificate
                .signers
                .binary_search(&responder)
                .is_err(),
            "fixture responder must be in the roster but outside the certified signer set"
        );
        let response =
            signed_certified_response(&request, manifest.clone(), body, responder, &keys);
        let persisted = PersistedCertifiedServePayloadV1 {
            format_version: FORMAT_VERSION,
            context_id: verified.context().id(),
            height: verified.context().height,
            request_hash: request.request_hash(),
            request: request.request().clone(),
            state: PersistedCertifiedServePayloadStateV1::Completed {
                response_hash: HashOf::new(&response),
                manifest,
                responder,
                signature: response.signature,
            },
        };
        let recovery = CertifiedServePayloadRecoveryCut {
            context_id: verified.context().id(),
            height: verified.context().height,
            payloads: BTreeMap::from([(persisted.id(), persisted)]),
        };
        assert!(matches!(
            recovery.authenticate_for_complete_tip_retirement(&verified, &keys[0]),
            Err(CertifiedServePayloadRecoveryError::InvalidResponse(message))
                if message.contains("lost certified local retention authority")
        ));
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
    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn rollback_journal_survives_restart_and_exact_retry_reactivates_it() {
        let temporary = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"removed!".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let pending = store
            .persist_pending(&request)
            .expect("persist Pending frame");
        let canonical = store.path_for(pending.id());
        let removed = store.removal_path_for(pending.id());
        let canonical_frame = fs::read(&canonical).expect("read canonical Pending frame");

        store
            .rollback_pending(pending)
            .expect("move Pending frame to removal journal");
        assert!(!canonical.exists());
        assert_eq!(
            fs::read(&removed).expect("read durable removal journal"),
            canonical_frame
        );
        drop(store);

        let (mut store, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), &context)
            .expect("authenticate removal journal after restart");
        assert!(recovery.is_empty());
        let retried = store
            .persist_pending(&request)
            .expect("reactivate exact removed request");
        assert!(!removed.exists());
        assert_eq!(
            fs::read(&canonical).expect("read reactivated Pending frame"),
            canonical_frame
        );
        store
            .rollback_pending(retried)
            .expect("reuse deterministic removal journal name");
        assert!(!canonical.exists());
        assert_eq!(
            fs::read(&removed).expect("read reused removal journal"),
            canonical_frame
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
    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn reopen_quarantines_a_synced_pending_stage_without_fabricating_admission() {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"recover!".to_vec());
        let (store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let id = CertifiedServePayloadId::from_request(request.request());
        let interrupted = store.temporary_path(id);
        let quarantine = store.quarantine_path(id);
        let pending = PersistedCertifiedServePayloadV1 {
            format_version: FORMAT_VERSION,
            context_id: context.id(),
            height: context.height,
            request_hash: id.request_hash(),
            request: request.request().clone(),
            state: PersistedCertifiedServePayloadStateV1::Pending,
        };
        let (pending_frame, _) = encode_frame(&pending, store.max_entry_bytes)
            .expect("encode interrupted Pending frame");
        drop(store);
        fs::write(&interrupted, &pending_frame).expect("write interrupted Pending fixture");
        fs::set_permissions(&interrupted, fs::Permissions::from_mode(PRIVATE_LEAF_MODE))
            .expect("set interrupted frame private");
        File::open(&interrupted)
            .and_then(|file| file.sync_all())
            .expect("synchronise interrupted Pending fixture");
        let (store, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), &context)
            .expect("quarantine interrupted Pending stage");
        assert!(!interrupted.exists());
        assert_eq!(
            fs::read(&quarantine).expect("read quarantined Pending stage"),
            pending_frame
        );
        assert!(recovery.is_empty());
        let mut store = store;
        let durable_pending = store
            .persist_pending(&request)
            .expect("retry admission after inert quarantine");
        let final_path = store.path_for(durable_pending.id());
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

    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn reopen_quarantines_a_partial_stage_and_detects_later_quarantine_drift() {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"partial!".to_vec());
        let (store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let id = CertifiedServePayloadId::from_request(request.request());
        let interrupted = store.temporary_path(id);
        let quarantine = store.quarantine_path(id);
        drop(store);
        let partial = b"truncated";
        fs::write(&interrupted, partial).expect("write partial stage");
        fs::set_permissions(&interrupted, fs::Permissions::from_mode(PRIVATE_LEAF_MODE))
            .expect("set partial stage private");
        File::open(&interrupted)
            .and_then(|file| file.sync_all())
            .expect("synchronise partial stage");

        let (store, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), &context)
            .expect("quarantine partial stage");
        assert!(recovery.is_empty());
        assert!(!interrupted.exists());
        assert_eq!(
            fs::read(&quarantine).expect("read quarantined partial stage"),
            partial
        );
        fs::write(&quarantine, b"poisoned!").expect("drift quarantined stage in place");
        assert!(matches!(
            store.reload_payload_census_strict(),
            Err(CertifiedServePayloadStoreError::AuthenticatedRecoveryCutMismatch)
        ));
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn repeated_stage_crashes_use_distinct_bounded_quarantine_slots() {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = TempDir::new().expect("temporary repeated-crash directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"repeat-crash".to_vec());
        let (store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let id = CertifiedServePayloadId::from_request(request.request());
        let interrupted = store.temporary_path(id);
        let first_quarantine = store.quarantine_path_for_slot(id, 0);
        let second_quarantine = store.quarantine_path_for_slot(id, 1);
        drop(store);

        let first_partial = b"first-partial-stage";
        fs::write(&interrupted, first_partial).expect("write first partial stage");
        fs::set_permissions(&interrupted, fs::Permissions::from_mode(PRIVATE_LEAF_MODE))
            .expect("make first stage private");
        File::open(&interrupted)
            .and_then(|file| file.sync_all())
            .expect("synchronise first partial stage");
        let (mut store, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), &context)
            .expect("quarantine first interrupted stage");
        assert!(recovery.is_empty());
        assert_eq!(
            fs::read(&first_quarantine).expect("read first quarantine slot"),
            first_partial
        );
        let _ = store
            .persist_pending(&request)
            .expect("retry and publish canonical Pending frame");
        drop(store);

        let second_partial = b"second-partial-stage";
        fs::write(&interrupted, second_partial).expect("write second partial stage");
        fs::set_permissions(&interrupted, fs::Permissions::from_mode(PRIVATE_LEAF_MODE))
            .expect("make second stage private");
        File::open(&interrupted)
            .and_then(|file| file.sync_all())
            .expect("synchronise second partial stage");
        let (_store, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), &context)
            .expect("quarantine a later interrupted stage without colliding");
        assert_eq!(
            fs::read(&first_quarantine).expect("reread first quarantine slot"),
            first_partial
        );
        assert_eq!(
            fs::read(&second_quarantine).expect("read second quarantine slot"),
            second_partial
        );
        assert!(matches!(
            recovery
                .get(id)
                .expect("recover canonical Pending frame")
                .state(),
            RecoveredCertifiedServePayloadState::Pending
        ));
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn full_quarantine_fails_closed_without_deleting_the_new_stage() {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = TempDir::new().expect("temporary full-quarantine directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"full-quarantine".to_vec());
        let (store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let id = CertifiedServePayloadId::from_request(request.request());
        let interrupted = store.temporary_path(id);
        let quarantine_paths = (0..MAX_QUARANTINED_STAGES_PER_HEIGHT)
            .map(|slot| store.quarantine_path_for_slot(id, slot))
            .collect::<Vec<_>>();
        drop(store);

        for (slot, path) in quarantine_paths.iter().enumerate() {
            fs::write(path, [u8::try_from(slot).expect("bounded slot")])
                .expect("write occupied quarantine slot");
            fs::set_permissions(path, fs::Permissions::from_mode(PRIVATE_LEAF_MODE))
                .expect("make occupied quarantine slot private");
        }
        fs::write(&interrupted, b"one-stage-too-many").expect("write new interrupted stage");
        fs::set_permissions(&interrupted, fs::Permissions::from_mode(PRIVATE_LEAF_MODE))
            .expect("make new stage private");

        assert!(matches!(
            CertifiedServePayloadStoreV1::open(temporary.path(), &context),
            Err(CertifiedServePayloadStoreError::DirectoryCapacityExceeded {
                capacity: MAX_QUARANTINED_STAGES_PER_HEIGHT
            })
        ));
        assert_eq!(
            fs::read(&interrupted).expect("reread retained overflow stage"),
            b"one-stage-too-many"
        );
        assert!(quarantine_paths.iter().all(|path| path.exists()));
    }

    #[cfg(all(unix, not(target_os = "espidf")))]
    #[test]
    fn reopen_resumes_an_exact_synced_terminal_stage() {
        use std::os::unix::fs::PermissionsExt as _;

        let temporary = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (request, _) = request_and_response(&context, &keys[0], 0, b"terminal".to_vec());
        let (mut store, _) =
            CertifiedServePayloadStoreV1::open(temporary.path(), &context).expect("open store");
        let pending = store
            .persist_pending(&request)
            .expect("persist Pending frame");
        let (mut terminal, _) = store
            .load_id_with_leaf(pending.id())
            .expect("load canonical Pending frame");
        let outcome = CertifiedServePayloadNegativeOutcome::Rejected(31);
        terminal.state = PersistedCertifiedServePayloadStateV1::Negative { outcome };
        let (terminal_frame, _) = encode_frame(&terminal, store.max_entry_bytes)
            .expect("encode interrupted terminal stage");
        let interrupted = store.temporary_path(pending.id());
        let terminal_path = store.terminal_path_for(pending.id());
        drop(store);
        fs::write(&interrupted, &terminal_frame).expect("write terminal stage");
        fs::set_permissions(&interrupted, fs::Permissions::from_mode(PRIVATE_LEAF_MODE))
            .expect("set terminal stage private");
        File::open(&interrupted)
            .and_then(|file| file.sync_all())
            .expect("synchronise terminal stage");

        let (_store, recovery) = CertifiedServePayloadStoreV1::open(temporary.path(), &context)
            .expect("resume exact terminal stage");
        assert!(!interrupted.exists());
        assert_eq!(
            fs::read(&terminal_path).expect("read resumed terminal companion"),
            terminal_frame
        );
        assert!(matches!(
            recovery.get(pending.id()).expect("recover terminal state").state(),
            RecoveredCertifiedServePayloadState::Negative(recovered) if recovered == outcome
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
        let unexpected = temporary.path().join(STORE_DIRECTORY).join("unexpected");
        fs::write(&unexpected, b"unexpected").expect("write unexpected fixture");
        #[cfg(all(unix, not(target_os = "espidf")))]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&unexpected, fs::Permissions::from_mode(PRIVATE_LEAF_MODE))
                .expect("set unexpected fixture private");
        }
        assert!(matches!(
            CertifiedServePayloadStoreV1::open(temporary.path(), &context),
            Err(CertifiedServePayloadStoreError::UnexpectedEntry(_))
        ));
    }
}
