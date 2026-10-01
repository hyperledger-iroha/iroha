//! Strict native frame contract for a qualified KAGEMUSHA Core coordinator.
//!
//! This module defines bounded framing, inventory and canonical public archive bindings.
//! The generic bridge does not synthesize a coordinator, monetary result, storage handle,
//! or hardware authority. Its exported open/invoke functions validate these inputs and
//! fail closed until a qualified platform build supplies the authenticated durable coordinator.
//!
//! The enrollment kernel is available to an independently provisioned Rust owner. Recovery
//! composes the concrete authenticated Core and its original journals. Actual monetary dispatch
//! additionally requires the durable production checkpoint and proof pipeline.

pub(crate) mod archive_boundary;
mod archives;
mod core_authorization_signer;
pub use core_authorization_signer::KagemushaNativeCoreAuthorizationSignerV1;
mod app_owned_identity_frames;
mod enrollment_attempt_journal;
mod enrollment_phase_one_backend;
mod exclusive_backend;
mod native_core_work;
mod native_installation;
#[cfg(unix)]
mod ordinary_app_identity;
pub use native_core_work::{
    KagemushaNativeCompletedOutboxReleaseLocatorV1, KagemushaNativeCorePublicationDestinationV1,
    KagemushaNativeCoreWorkSourceV1, KagemushaNativeIncomingEvidenceSourceV1,
    KagemushaNativeIncomingFoldOriginalsV1, KagemushaNativeIncomingStageOriginalsV1,
    KagemushaNativeOutboxReleaseOriginalsV1, KagemushaNativeOutgoingCompletionOriginalsV1,
    KagemushaNativeRedemptionFinalitySourceV1, KagemushaNativeSenderPreparationV1,
    register_kagemusha_native_core_work_source_v1,
    register_kagemusha_native_incoming_evidence_source_v1,
    register_kagemusha_native_redemption_finality_source_v1,
};
#[cfg(unix)]
pub use ordinary_app_identity::{
    KagemushaNativeOrdinaryAppIdentitySourceV1, KagemushaOrdinaryAppIdentityInstallErrorV1,
    KagemushaOrdinaryEnrollmentDispositionV1, bootstrap_kagemusha_native_ordinary_app_identity_v1,
    register_kagemusha_native_ordinary_app_identity_source_v1,
};
mod recovered_backend;
#[cfg(test)]
mod recovered_frame_tests;
pub use recovered_backend::KagemushaAuthenticatedRecoveredCoordinatorV1;
mod pre_enrollment_qualification;
mod qualified_enrollment_delegate;
pub use native_installation::{
    KagemushaNativeEnrollmentOriginalCustodyV1, KagemushaNativeEnrollmentProvisionerV1,
    KagemushaNativeEnrollmentProvisioningV1, KagemushaNativeProvisionerInstallErrorV1,
    KagemushaPinnedNativeEnrollmentProvisionerV1,
    provision_and_install_kagemusha_native_enrollment_v1,
    register_kagemusha_native_enrollment_provisioner_v1,
};
pub use pre_enrollment_qualification::{
    KagemushaPreEnrollmentQualificationOwnerV1, KagemushaVerifiedPreEnrollmentQualificationV1,
};
mod signed_app_preparation;
pub use enrollment_attempt_journal::{
    KagemushaEnrollmentAttemptJournalV1, KagemushaEnrollmentJournalDispatchV1,
    KagemushaEnrollmentJournalErrorV1, KagemushaEnrollmentJournalPinsV1,
    KagemushaEnrollmentJournalReservationV1, KagemushaEnrollmentJournalResultV1,
    KagemushaEnrollmentJournalSelectionV1, KagemushaEnrollmentJournalStoreV1,
    KagemushaEnrollmentLiveSelectionV1,
};
pub use enrollment_phase_one_backend::{
    KagemushaEnrollmentPhaseOneBackendV1, KagemushaQualifiedEnrollmentDelegateV1,
};
pub(crate) use exclusive_backend::{
    KagemushaCheckedCoordinatorErrorV1, KagemushaExclusiveCoordinatorBackendV1,
};
pub use initial_enrollment::{
    AcceptedIssuerChallengeV1, FreshIssuerAdmissionV1, InitialEnrollmentErrorV1,
    IssuerChallengeProjectionV1, PendingIssuerEnrollmentV1, PreparedIssuerProofV1,
};
pub use qualified_enrollment_delegate::{
    KagemushaEnrollmentContextProviderV1, KagemushaEnrollmentProvisionedContextV1,
    KagemushaKernelEnrollmentDelegateV1,
};
pub use signed_app_preparation::{
    SignedAppPreparationErrorV1, SignedAppPreparationPinsV1, VerifiedSignedAppPreparationV1,
    verify_signed_app_preparation_v1,
};
// Production recovery uses only the concrete authenticated Core owner. Structural owners
// remain inside the test modules and cannot supply the installed native selection.
mod enrolled_open;
mod enrolled_session;
mod native_core_bootstrap;
mod sender_observation;
pub use native_core_bootstrap::{
    KagemushaNativeCoreBootstrapInputsV1, KagemushaNativeCoreBootstrapSourceV1,
    KagemushaNativeFreshCoreBootstrapV1,
};
mod initial_enrollment;
pub(crate) mod native_deadline;
mod session_registry;
pub(crate) mod startup_qualification;
pub use crate::kagemusha_device_bridge_v1::sender_payload::{
    SenderPreparationSelectorV1 as KagemushaCoreSenderPreparationSelectorV1,
    SenderWalletContextV1 as KagemushaCoreSenderWalletContextV1,
};
pub use archives::{
    KAGEMUSHA_CORE_COORDINATOR_ARCHIVE_MAX_BYTES_V1, KagemushaCoreCoordinatorArchiveErrorV1,
    KagemushaCoreSenderCandidateArchiveV1, KagemushaCoreSenderPreparationArchiveV1,
    KagemushaCoreSenderRecoveryArchiveV1,
};

use iroha_data_model::kagemusha::{
    KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1, KagemushaArtifactRoleV1,
    KagemushaDeviceSignatureV1, KagemushaHardwareSelectionSigningLayoutV1,
    KagemushaQualifiedHelperCircuitV1, KagemushaQualifiedRelationV1,
};
use sha2::{Digest as _, Sha256};
use std::sync::{Arc, OnceLock};

use crate::{
    CONNECT_NORITO_BRIDGE_ABI_VERSION, CONNECT_NORITO_KAGEMUSHA_IPM1_MESSAGE_KIND_TAGS_V1,
    KAGEMUSHA_NATIVE_HARDWARE_CAPABILITY_BITS_V1, KagemushaDeviceLifecycleOperationV1,
};

/// Magic prefix on each coordinator request and response frame.
pub const KAGEMUSHA_CORE_COORDINATOR_FRAME_MAGIC_V1: [u8; 8] = *b"IKGMCOR1";
/// Sole supported coordinator frame version.
// Schema 2 is the sole coordinator frame schema. V1 names refer to the monetary protocol,
// not a compatibility decoder for the retired two-field reservation request.
pub const KAGEMUSHA_CORE_COORDINATOR_FRAME_VERSION_V1: u16 = 2;
/// Fixed bytes before length-prefixed frame fields.
pub const KAGEMUSHA_CORE_COORDINATOR_FRAME_HEADER_BYTES_V1: usize = 16;
/// Maximum number of fields in one coordinator frame.
pub const KAGEMUSHA_CORE_COORDINATOR_MAX_FIELDS_V1: usize = 16;
/// Maximum bytes in one coordinator frame field.
pub const KAGEMUSHA_CORE_COORDINATOR_MAX_FIELD_BYTES_V1: usize = 96 * 1024;
/// Maximum complete request-frame bytes.
pub const KAGEMUSHA_CORE_COORDINATOR_MAX_REQUEST_BYTES_V1: usize = 256 * 1024;
/// Maximum complete response-frame bytes.
pub const KAGEMUSHA_CORE_COORDINATOR_MAX_RESPONSE_BYTES_V1: usize = 128 * 1024;
/// Maximum UTF-8 durable-store path accepted by the native boundary.
pub const KAGEMUSHA_CORE_COORDINATOR_MAX_STORAGE_PATH_BYTES_V1: usize = 4 * 1024;
/// Number of complete public wire payloads in V1.
pub const KAGEMUSHA_CORE_COORDINATOR_WIRE_PAYLOAD_COUNT_V1: u32 = 6;
/// RecoverSender selector for lookup by an already exposed terminal identity.
pub const KAGEMUSHA_CORE_COORDINATOR_RECOVER_BY_TERMINAL_ID_V1: u8 = 0;
/// RecoverSender selector for lookup by the caller-persisted operation identity.
pub const KAGEMUSHA_CORE_COORDINATOR_RECOVER_BY_OPERATION_ID_V1: u8 = 1;

const KAGEMUSHA_CORE_COORDINATOR_PROTOCOL_VERSION_V1: u32 = 1;
const KAGEMUSHA_CORE_COORDINATOR_COMPLETE_CAPABILITY_MASK_V1: u32 = 0xffff;
const KAGEMUSHA_CORE_COORDINATOR_SEND_SPLIT_V1: u32 = 0;
const KAGEMUSHA_CORE_COORDINATOR_REDEEM_SPLIT_V1: u32 = 1;
const KAGEMUSHA_CORE_COORDINATOR_QUALIFICATION_FIELDS_V1: usize = 5;
const INITIAL_ENROLLMENT_BEGIN_V1: u32 = 1;
const INITIAL_ENROLLMENT_ACCEPT_CHALLENGE_V1: u32 = 2;
const INITIAL_ENROLLMENT_PREPARE_PROOF_V1: u32 = 3;
const INITIAL_ENROLLMENT_READ_PROOF_V1: u32 = 4;
const INITIAL_ENROLLMENT_COMPLETE_V1: u32 = 5;
const INITIAL_ENROLLMENT_CANCEL_V1: u32 = 6;
const INITIAL_ENROLLMENT_READ_SELECTION_V1: u32 = 7;
const INITIAL_ENROLLMENT_VERIFY_APP_PREPARATION_V1: u32 = 8;
const APP_ATTEST_SELECTION_SIGNING_DOMAIN_V1: &[u8] =
    b"iroha:kagemusha:v1:hardware-transition-selection\0";
const APP_ATTEST_SELECTION_BODY_BYTES_V1: u64 = 403;
const APP_ATTEST_SELECTION_SIGNING_BYTES_V1: usize = 460;

/// Exact native coordinator contract returned as twelve `u32` words.
pub const KAGEMUSHA_CORE_COORDINATOR_CONTRACT_WORDS_V1: [u32; 12] = [
    KAGEMUSHA_CORE_COORDINATOR_FRAME_VERSION_V1 as u32,
    CONNECT_NORITO_BRIDGE_ABI_VERSION,
    CONNECT_NORITO_KAGEMUSHA_IPM1_MESSAGE_KIND_TAGS_V1.len() as u32,
    KAGEMUSHA_CORE_COORDINATOR_WIRE_PAYLOAD_COUNT_V1,
    KagemushaArtifactRoleV1::ALL.len() as u32,
    KagemushaQualifiedRelationV1::ALL.len() as u32,
    KagemushaQualifiedHelperCircuitV1::ALL.len() as u32,
    KagemushaDeviceLifecycleOperationV1::ALL.len() as u32,
    KAGEMUSHA_NATIVE_HARDWARE_CAPABILITY_BITS_V1.len() as u32,
    KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1,
    1, // Required native close/revocation lifecycle.
    KagemushaCoreCoordinatorMethodV1::ALL.len() as u32,
];

// The read-only Core archive export and the pinned testnet observer accept the same public input.
const _: () = assert!(
    iroha_core_zk::kagemusha_v1_state::KAGEMUSHA_OUTGOING_STATE_PUBLIC_INPUT_ARCHIVE_MAX_BYTES_V1
        == crate::kagemusha_testnet_observation_v1::KAGEMUSHA_TESTNET_STATE_INPUT_MAX_BYTES_V1
);

/// Closed coordinator method inventory.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaCoreCoordinatorMethodV1 {
    /// Reserve a fresh durable operation identity.
    ReserveOperationId = 1,
    /// Accept a fully authenticated qualification binding.
    AcceptQualification = 2,
    /// Accept an authenticated secure-device reply.
    AcceptAuthenticatedReply = 3,
    /// Begin an exact-next sender transition.
    BeginSenderTransition = 4,
    /// Prove the already prepared sender transition.
    ProvePreparedSenderTransition = 5,
    /// Build the exact terminal envelope.
    BuildTerminalEnvelope = 6,
    /// Accept an installed terminal envelope.
    AcceptInstalledTerminal = 7,
    /// Recover the authenticated sender operation index.
    RecoverSender = 8,
    /// Recover the byte-identical terminal envelope.
    RecoverTerminalEnvelope = 9,
    /// Release an outbox tombstone after a closed terminal receipt.
    ReleaseOutbox = 10,
    /// Begin a transient read using a native-generated observation challenge.
    BeginObservation = 11,
    /// Advance the initial issuer ceremony or separate recovered possession on the sole
    /// installed owner. The concrete backend retains the opaque phase, authenticates exact
    /// originals and rejects substituted retry proofs, source owners and authority pins.
    InitialEnrollment = 12,
    /// Acknowledge an App Attest assertion only after the original terminal is durably committed.
    AcknowledgeCommittedAppAttest = 13,
    /// Read the original paired outgoing State proof retained by the authenticated Core owner.
    ExportOutgoingStateProof = 14,
    /// Prepare and durably retain the actual native incoming proof before physical fold work.
    PrepareIncomingFold = 15,
    /// Consume the same original incoming proof/Guard/device selection and finish publication.
    CompleteIncomingFold = 16,
    /// Stage independently selected raw mint/peer originals under actual native authority.
    StageIncomingOriginal = 17,
    /// Read the authenticated installed release and distinct provider policy root.
    AuthenticatedHardwarePolicy = 18,
    /// Prepare or consume an ordinary app operation approval, independently of monetary proof.
    PreparedAppOperationApproval = 19,
    /// Prove initial ordinary app-key possession after genuine pending raw admission.
    PreparedAppEnrollmentPossession = 20,
    /// Prepare and retain ordinary app identity with separate generation/attestation fences.
    PreparedOrdinaryAppIdentity = 21,
}

impl KagemushaCoreCoordinatorMethodV1 {
    /// All coordinator methods in canonical code order.
    pub const ALL: [Self; 21] = [
        Self::ReserveOperationId,
        Self::AcceptQualification,
        Self::AcceptAuthenticatedReply,
        Self::BeginSenderTransition,
        Self::ProvePreparedSenderTransition,
        Self::BuildTerminalEnvelope,
        Self::AcceptInstalledTerminal,
        Self::RecoverSender,
        Self::RecoverTerminalEnvelope,
        Self::ReleaseOutbox,
        Self::BeginObservation,
        Self::InitialEnrollment,
        Self::AcknowledgeCommittedAppAttest,
        Self::ExportOutgoingStateProof,
        Self::PrepareIncomingFold,
        Self::CompleteIncomingFold,
        Self::StageIncomingOriginal,
        Self::AuthenticatedHardwarePolicy,
        Self::PreparedAppOperationApproval,
        Self::PreparedAppEnrollmentPossession,
        Self::PreparedOrdinaryAppIdentity,
    ];

    /// Parse one closed coordinator method code.
    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::ReserveOperationId),
            2 => Some(Self::AcceptQualification),
            3 => Some(Self::AcceptAuthenticatedReply),
            4 => Some(Self::BeginSenderTransition),
            5 => Some(Self::ProvePreparedSenderTransition),
            6 => Some(Self::BuildTerminalEnvelope),
            7 => Some(Self::AcceptInstalledTerminal),
            8 => Some(Self::RecoverSender),
            9 => Some(Self::RecoverTerminalEnvelope),
            10 => Some(Self::ReleaseOutbox),
            11 => Some(Self::BeginObservation),
            12 => Some(Self::InitialEnrollment),
            13 => Some(Self::AcknowledgeCommittedAppAttest),
            14 => Some(Self::ExportOutgoingStateProof),
            15 => Some(Self::PrepareIncomingFold),
            16 => Some(Self::CompleteIncomingFold),
            17 => Some(Self::StageIncomingOriginal),
            18 => Some(Self::AuthenticatedHardwarePolicy),
            19 => Some(Self::PreparedAppOperationApproval),
            20 => Some(Self::PreparedAppEnrollmentPossession),
            21 => Some(Self::PreparedOrdinaryAppIdentity),
            _ => None,
        }
    }

    /// Return this method's one-byte code.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }
}

/// Closed frame-validation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaCoreCoordinatorFrameErrorV1 {
    /// Frame length is empty, truncated, oversized, or arithmetically invalid.
    Size,
    /// Magic, version, field count, or reserved bytes are invalid.
    Header,
    /// A length-prefixed field is truncated or exceeds its per-field bound.
    Field,
    /// Bytes remain after the declared final field.
    TrailingBytes,
}

/// Failure returned by a qualified platform coordinator backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaCoreCoordinatorBackendErrorV1 {
    /// Qualified hardware or its authenticated durable store is unavailable.
    Unavailable,
    /// The authenticated coordinator rejected the requested operation.
    Rejected,
}

/// Install-time failure for the process-global qualified coordinator backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaCoreCoordinatorInstallErrorV1 {
    /// A backend was already installed and cannot be replaced or removed.
    AlreadyInstalled,
}

/// Qualified platform implementation behind the KAGEMUSHA coordinator ABI.
///
/// Implementations must bind `open` to authenticated durable state and `invoke`
/// to the non-forking hardware provider. The bridge validates the storage path,
/// method code, and request frame before dispatch, then validates the complete
/// response frame before it can cross the ABI.
pub trait KagemushaCoreCoordinatorBackendV1: Send + Sync + 'static {
    /// Open an authenticated durable store and return a nonzero opaque handle.
    fn open(&self, storage_path: &str) -> Result<u64, KagemushaCoreCoordinatorBackendErrorV1>;

    /// Invoke one closed method with an already validated canonical request frame.
    fn invoke(
        &self,
        handle: u64,
        method: KagemushaCoreCoordinatorMethodV1,
        request_frame: &[u8],
    ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1>;

    /// Run one initial-enrollment phase using the backend's pinned issuer, verifier,
    /// release and retained nonserializable native attempt. Existing coordinators
    /// remain unavailable unless a qualified implementation explicitly overrides this.
    fn invoke_initial_enrollment(
        &self,
        _handle: u64,
        _request_frame: &[u8],
    ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1> {
        Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable)
    }

    /// Return the exact bound acknowledgment only after independently authenticating the
    /// installed terminal journal, enrolled App Attest key, original selection, raw assertion
    /// signature and exact-next counter. It must be idempotent for the original operation and
    /// reject every substituted or merely prepared terminal. Stock backends remain unavailable.
    /// TODO: Install a phone-qualified durable Core backend before production admission.
    fn acknowledge_committed_app_attest(
        &self,
        _handle: u64,
        _request_frame: &[u8],
    ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1> {
        Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable)
    }

    /// Export the original bounded paired State proof from this owner's authenticated Core
    /// operation index. Implementations must call Core's retained archive exporter rather than
    /// reconstructing a statement from app bytes. This read confers no monetary authority.
    fn export_outgoing_state_proof(
        &self,
        _handle: u64,
        _operation_id: [u8; 32],
    ) -> Result<
        iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingStateProofArchivePairV1,
        KagemushaCoreCoordinatorBackendErrorV1,
    > {
        Err(KagemushaCoreCoordinatorBackendErrorV1::Unavailable)
    }

    /// Tear down the selected hardware session after the bridge revokes its handle.
    /// This is not an operation to commit, abort or erase uncertain monetary state.
    fn close(&self, handle: u64) -> Result<(), KagemushaCoreCoordinatorBackendErrorV1>;
}

static KAGEMUSHA_CORE_COORDINATOR_BACKEND_V1: OnceLock<
    Arc<KagemushaExclusiveCoordinatorBackendV1>,
> = OnceLock::new();

/// Install the qualified coordinator backend exactly once for this process.
///
/// The installed backend is always wrapped in a process-exclusive owner: one
/// attempted open and serialized invocations on its original handle. Stock
/// builds call it only through the independently retained native provisioner. There is no
/// uninstall, overwrite, software substitute or C/JNI intake for a backend or authority.
pub fn install_kagemusha_core_coordinator_backend_v1(
    backend: Arc<dyn KagemushaCoreCoordinatorBackendV1>,
) -> Result<(), KagemushaCoreCoordinatorInstallErrorV1> {
    let exclusive = Arc::new(KagemushaExclusiveCoordinatorBackendV1::new(backend));
    KAGEMUSHA_CORE_COORDINATOR_BACKEND_V1
        .set(exclusive)
        .map_err(|_| KagemushaCoreCoordinatorInstallErrorV1::AlreadyInstalled)
}

pub(crate) fn installed_kagemusha_core_coordinator_backend_v1()
-> Option<&'static KagemushaExclusiveCoordinatorBackendV1> {
    KAGEMUSHA_CORE_COORDINATOR_BACKEND_V1.get().map(Arc::as_ref)
}

fn encode_frame(
    fields: &[Vec<u8>],
    maximum: usize,
) -> Result<Vec<u8>, KagemushaCoreCoordinatorFrameErrorV1> {
    if fields.len() > KAGEMUSHA_CORE_COORDINATOR_MAX_FIELDS_V1 {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Header);
    }
    let mut total = KAGEMUSHA_CORE_COORDINATOR_FRAME_HEADER_BYTES_V1;
    for field in fields {
        if field.len() > KAGEMUSHA_CORE_COORDINATOR_MAX_FIELD_BYTES_V1 {
            return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
        }
        total = total
            .checked_add(4)
            .and_then(|length| length.checked_add(field.len()))
            .ok_or(KagemushaCoreCoordinatorFrameErrorV1::Size)?;
    }
    if total > maximum {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Size);
    }

    let mut frame = Vec::with_capacity(total);
    frame.extend_from_slice(&KAGEMUSHA_CORE_COORDINATOR_FRAME_MAGIC_V1);
    frame.extend_from_slice(&KAGEMUSHA_CORE_COORDINATOR_FRAME_VERSION_V1.to_le_bytes());
    frame.extend_from_slice(&(fields.len() as u16).to_le_bytes());
    frame.extend_from_slice(&0_u32.to_le_bytes());
    for field in fields {
        frame.extend_from_slice(&(field.len() as u32).to_le_bytes());
        frame.extend_from_slice(field);
    }
    Ok(frame)
}

fn decode_frame(
    frame: &[u8],
    maximum: usize,
) -> Result<Vec<Vec<u8>>, KagemushaCoreCoordinatorFrameErrorV1> {
    if frame.len() < KAGEMUSHA_CORE_COORDINATOR_FRAME_HEADER_BYTES_V1 || frame.len() > maximum {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Size);
    }
    if frame[..8] != KAGEMUSHA_CORE_COORDINATOR_FRAME_MAGIC_V1
        || u16::from_le_bytes([frame[8], frame[9]]) != KAGEMUSHA_CORE_COORDINATOR_FRAME_VERSION_V1
    {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Header);
    }
    let field_count = usize::from(u16::from_le_bytes([frame[10], frame[11]]));
    if field_count > KAGEMUSHA_CORE_COORDINATOR_MAX_FIELDS_V1
        || frame[12..16] != 0_u32.to_le_bytes()
    {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Header);
    }

    let mut cursor = KAGEMUSHA_CORE_COORDINATOR_FRAME_HEADER_BYTES_V1;
    let mut fields = Vec::with_capacity(field_count);
    for _ in 0..field_count {
        let length_end = cursor
            .checked_add(4)
            .ok_or(KagemushaCoreCoordinatorFrameErrorV1::Size)?;
        let length_bytes = frame
            .get(cursor..length_end)
            .ok_or(KagemushaCoreCoordinatorFrameErrorV1::Field)?;
        let field_length = usize::try_from(u32::from_le_bytes(
            length_bytes
                .try_into()
                .expect("checked four-byte field length"),
        ))
        .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Size)?;
        if field_length > KAGEMUSHA_CORE_COORDINATOR_MAX_FIELD_BYTES_V1 {
            return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
        }
        cursor = length_end;
        let field_end = cursor
            .checked_add(field_length)
            .ok_or(KagemushaCoreCoordinatorFrameErrorV1::Size)?;
        let field = frame
            .get(cursor..field_end)
            .ok_or(KagemushaCoreCoordinatorFrameErrorV1::Field)?;
        fields.push(field.to_vec());
        cursor = field_end;
    }
    if cursor != frame.len() {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::TrailingBytes);
    }
    Ok(fields)
}

fn require_u32_range(
    field: Option<&Vec<u8>>,
    minimum: u32,
    maximum: u32,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    let bytes: [u8; 4] = field
        .ok_or(KagemushaCoreCoordinatorFrameErrorV1::Field)?
        .as_slice()
        .try_into()
        .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
    let value = u32::from_le_bytes(bytes);
    if value < minimum || value > maximum {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    Ok(())
}

/// Encode JNI request fields into the exact bounded coordinator frame.
pub fn kagemusha_core_coordinator_encode_request_v1(
    fields: &[Vec<u8>],
) -> Result<Vec<u8>, KagemushaCoreCoordinatorFrameErrorV1> {
    encode_frame(fields, KAGEMUSHA_CORE_COORDINATOR_MAX_REQUEST_BYTES_V1)
}

/// Decode an exact bounded coordinator request frame.
pub fn kagemusha_core_coordinator_decode_request_v1(
    frame: &[u8],
) -> Result<Vec<Vec<u8>>, KagemushaCoreCoordinatorFrameErrorV1> {
    decode_frame(frame, KAGEMUSHA_CORE_COORDINATOR_MAX_REQUEST_BYTES_V1)
}

/// Validate method-specific transport shapes and selectors.
///
/// C/JNI dispatch additionally applies the private typed archive boundary. Passing this
/// transport-only validator does not make opaque payload fields canonical or authenticated.
/// BeginSenderTransition field zero is always the caller-persisted nonzero operation ID.
/// RecoverSender fields zero and one are respectively the closed selector and selected nonzero ID.
/// All integer discriminants use canonical little-endian `u32` fields, matching both signed apps.
pub fn kagemusha_core_coordinator_validate_method_request_v1(
    method: KagemushaCoreCoordinatorMethodV1,
    frame: &[u8],
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    let fields = kagemusha_core_coordinator_decode_request_v1(frame)?;
    match method {
        KagemushaCoreCoordinatorMethodV1::PreparedAppOperationApproval
        | KagemushaCoreCoordinatorMethodV1::PreparedAppEnrollmentPossession
        | KagemushaCoreCoordinatorMethodV1::PreparedOrdinaryAppIdentity => {
            app_owned_identity_frames::validate_request(method, &fields)
        }
        KagemushaCoreCoordinatorMethodV1::AuthenticatedHardwarePolicy => {
            require_field_count(&fields, 0)
        }
        KagemushaCoreCoordinatorMethodV1::ReserveOperationId => {
            require_field_count(&fields, 3)?;
            require_device_operation_field(fields.first())?;
            if is_observation_operation_v1(require_u32_field(fields.first())?) {
                return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
            }
            require_nonzero_digest_field(fields.get(1))?;
            require_nonempty_field(fields.get(2))
        }
        KagemushaCoreCoordinatorMethodV1::BeginObservation => {
            require_field_count(&fields, 2)?;
            if !is_observation_operation_v1(require_u32_field(fields.first())?) {
                return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
            }
            require_nonempty_field(fields.get(1))
        }
        KagemushaCoreCoordinatorMethodV1::ExportOutgoingStateProof => {
            require_field_count(&fields, 1)?;
            require_nonzero_digest_field(fields.first())
        }
        KagemushaCoreCoordinatorMethodV1::AcceptQualification => {
            require_field_count(&fields, 6)?;
            require_qualification_fields(&fields, 0)?;
            require_nonzero_digest_field(fields.get(5))
        }
        KagemushaCoreCoordinatorMethodV1::AcceptAuthenticatedReply => {
            require_device_operation_field(fields.first())?;
            let operation = require_u32_field(fields.first())?;
            require_field_count(&fields, if operation == 12 { 11 } else { 10 })?;
            require_nonzero_digest_field(fields.get(1))?;
            require_nonempty_field(fields.get(2))?;
            require_nonempty_field(fields.get(3))?;
            require_device_signature_field(fields.get(4))?;
            require_qualification_fields(&fields, 5)?;
            if operation == 12 {
                let request_id = fields[1]
                    .as_slice()
                    .try_into()
                    .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
                sender_observation::require_original_response(
                    12,
                    request_id,
                    &fields[3],
                    &fields[4],
                    &fields[10],
                )
                .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
            }
            Ok(())
        }
        KagemushaCoreCoordinatorMethodV1::BeginSenderTransition => {
            require_nonzero_digest_field(fields.first())?;
            let (_, qualification_start) = require_sender_input_fields(&fields, 1)?;
            require_field_count(
                &fields,
                qualification_start + KAGEMUSHA_CORE_COORDINATOR_QUALIFICATION_FIELDS_V1,
            )?;
            require_qualification_fields(&fields, qualification_start)
        }
        KagemushaCoreCoordinatorMethodV1::BuildTerminalEnvelope => {
            require_field_count(&fields, 2)?;
            let candidate =
                archives::KagemushaCoreSenderCandidateArchiveV1::decode_canonical_exact(&fields[0])
                    .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
            iroha_data_model::kagemusha::kagemusha_decode_device_success_response_v1(
                &fields[1],
                7,
                candidate.preparation.operation_id,
            )
            .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
            Ok(())
        }
        KagemushaCoreCoordinatorMethodV1::PrepareIncomingFold
        | KagemushaCoreCoordinatorMethodV1::StageIncomingOriginal => {
            require_field_count(&fields, 2)?;
            let maximum = if method == KagemushaCoreCoordinatorMethodV1::PrepareIncomingFold {
                1
            } else {
                2
            };
            require_u32_range(fields.first(), 0, maximum)?;
            require_nonzero_digest_field(fields.get(1))
        }
        KagemushaCoreCoordinatorMethodV1::CompleteIncomingFold => {
            require_field_count(&fields, 4)?;
            require_nonzero_digest_field(fields.first())?;
            for (index, maximum) in [
                (1, 8192),
                (2, KAGEMUSHA_CORE_COORDINATOR_MAX_FIELD_BYTES_V1),
            ] {
                require_nonempty_field(fields.get(index))?;
                if fields[index].len() > maximum {
                    return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
                }
            }
            require_canonical_paired_proof(&fields[1])?;
            if fields[3].len() != 64
                || iroha_data_model::kagemusha::KagemushaDeviceSignatureV1::from_raw_bytes(
                    &fields[3],
                )
                .is_err()
            {
                return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
            }
            Ok(())
        }
        KagemushaCoreCoordinatorMethodV1::ProvePreparedSenderTransition
        | KagemushaCoreCoordinatorMethodV1::RecoverTerminalEnvelope => {
            require_field_count(&fields, 2)?;
            require_nonempty_field(fields.first())?;
            require_nonempty_field(fields.get(1))
        }
        KagemushaCoreCoordinatorMethodV1::AcceptInstalledTerminal => {
            require_field_count(&fields, 5)?;
            for field in &fields {
                require_nonempty_field(Some(field))?;
            }
            Ok(())
        }
        KagemushaCoreCoordinatorMethodV1::RecoverSender => {
            require_field_count(&fields, 8)?;
            let selector = fields
                .first()
                .ok_or(KagemushaCoreCoordinatorFrameErrorV1::Field)?;
            if selector.as_slice() != [KAGEMUSHA_CORE_COORDINATOR_RECOVER_BY_TERMINAL_ID_V1]
                && selector.as_slice() != [KAGEMUSHA_CORE_COORDINATOR_RECOVER_BY_OPERATION_ID_V1]
            {
                return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
            }
            require_nonzero_digest_field(fields.get(1))?;
            require_sender_kind_field(fields.get(2))?;
            require_qualification_fields(&fields, 3)
        }
        KagemushaCoreCoordinatorMethodV1::ReleaseOutbox => {
            require_nonzero_digest_field(fields.first())?;
            let (sender_kind, terminal_start) = require_sender_input_fields(&fields, 1)?;
            require_field_count(
                &fields,
                terminal_start + 2 + KAGEMUSHA_CORE_COORDINATOR_QUALIFICATION_FIELDS_V1,
            )?;
            require_nonempty_field(fields.get(terminal_start))?;
            require_terminal_receipt_field(fields.get(terminal_start + 1), sender_kind)?;
            require_qualification_fields(&fields, terminal_start + 2)
        }
        KagemushaCoreCoordinatorMethodV1::InitialEnrollment => {
            match require_u32_field(fields.first())? {
                INITIAL_ENROLLMENT_BEGIN_V1 | INITIAL_ENROLLMENT_READ_SELECTION_V1 => {
                    require_field_count(&fields, 2)?;
                    require_bounded_nonempty_field(fields.get(1), 512)
                }
                INITIAL_ENROLLMENT_VERIFY_APP_PREPARATION_V1 => {
                    require_field_count(&fields, 3)?;
                    require_nonzero_ticket_field(fields.get(1))?;
                    require_exact_length_field(fields.get(2), 273)
                }
                9 => require_field_count(&fields, 1),
                10 => {
                    require_field_count(&fields, 4)?;
                    require_nonzero_ticket_field(fields.get(1))?;
                    require_exact_length_field(fields.get(2), 64)?;
                    require_bounded_nonempty_field(
                        fields.get(3),
                        iroha_data_model::kagemusha::KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1,
                    )
                }
                11 => {
                    require_field_count(&fields, 2)?;
                    require_nonzero_ticket_field(fields.get(1))
                }
                INITIAL_ENROLLMENT_ACCEPT_CHALLENGE_V1 => {
                    require_field_count(&fields, 11)?;
                    require_nonzero_ticket_field(fields.get(1))?;
                    require_exact_length_field(fields.get(2), 273)?;
                    require_bounded_nonempty_field(fields.get(3), 8 * 1024)?;
                    require_bounded_nonempty_field(fields.get(4), 2 * 1024)?;
                    require_bounded_nonempty_field(fields.get(5), 16 * 1024)?;
                    for index in 6..=8 {
                        require_nonzero_digest_field(fields.get(index))?;
                    }
                    require_bounded_nonempty_field(fields.get(9), 2 * 1024)?;
                    require_nonzero_ticket_field(fields.get(10))
                }
                INITIAL_ENROLLMENT_PREPARE_PROOF_V1 => {
                    require_field_count(&fields, 4)?;
                    require_nonzero_ticket_field(fields.get(1))?;
                    require_exact_length_field(fields.get(2), 64)?;
                    require_bounded_nonempty_field(
                        fields.get(3),
                        iroha_data_model::kagemusha::KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1,
                    )
                }
                INITIAL_ENROLLMENT_READ_PROOF_V1 | INITIAL_ENROLLMENT_CANCEL_V1 => {
                    require_field_count(&fields, 2)?;
                    require_nonzero_ticket_field(fields.get(1))
                }
                INITIAL_ENROLLMENT_COMPLETE_V1 => {
                    require_field_count(&fields, 3)?;
                    require_nonzero_ticket_field(fields.get(1))?;
                    require_bounded_nonempty_field(fields.get(2), 16 * 1024)
                }
                _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
            }
        }
        KagemushaCoreCoordinatorMethodV1::AcknowledgeCommittedAppAttest => {
            require_field_count(&fields, 7)?;
            require_nonzero_digest_field(fields.first())?;
            let key_id = fields
                .get(1)
                .ok_or(KagemushaCoreCoordinatorFrameErrorV1::Field)?;
            require_bounded_nonempty_field(Some(key_id), 512)?;
            if key_id.contains(&0) || core::str::from_utf8(key_id).is_err() {
                return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
            }
            let selection = fields
                .get(2)
                .ok_or(KagemushaCoreCoordinatorFrameErrorV1::Field)?;
            require_bounded_nonempty_field(fields.get(3), 8 * 1024)?;
            let previous = require_u32_field(fields.get(4))?;
            if previous == u32::MAX {
                return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
            }
            require_app_attest_selection_subject_v1(selection)?;
            let actual =
                iroha_data_model::kagemusha::kagemusha_app_attest_original_counter_v1(&fields[3])
                    .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
            if actual <= previous {
                return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
            }
            require_nonzero_digest_field(fields.get(5))?;
            require_nonzero_digest_field(fields.get(6))
        }
    }
}

fn require_app_attest_selection_subject_v1(
    selection: &[u8],
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    use KagemushaHardwareSelectionSigningLayoutV1 as S;

    if selection.len() != S::TOTAL_BYTES
        || selection.len() != APP_ATTEST_SELECTION_SIGNING_BYTES_V1
        || !selection.starts_with(APP_ATTEST_SELECTION_SIGNING_DOMAIN_V1)
        || selection[S::BODY_LENGTH] != APP_ATTEST_SELECTION_BODY_BYTES_V1.to_le_bytes()
        || selection[S::VERSION] != 1_u16.to_le_bytes()
    {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    let nonzero = |range: core::ops::Range<usize>| selection[range].iter().any(|byte| *byte != 0);
    if [
        S::RELEASE_ID,
        S::PROVIDER_POLICY_ROOT,
        S::APP_POLICY_DIGEST,
        S::CREDENTIAL_ID,
        S::NETWORK_ID,
        S::LANE_COMMITMENT,
        S::HARDWARE_PROFILE_ID,
        S::HARDWARE_EPOCH_ID,
        S::TRANSITION_STATEMENT_DIGEST,
    ]
    .into_iter()
    .any(|range| !nonzero(range))
        || !nonzero(S::POLICY_EPOCH)
        || !nonzero(S::HARDWARE_EPOCH_GENERATION)
    {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    let operation = selection[S::OPERATION_TAG.start];
    let outgoing = matches!(operation, 2 | 4);
    let before = u128::from_le_bytes(
        selection[S::SECURE_INDEX_BEFORE]
            .try_into()
            .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?,
    );
    let after = u128::from_le_bytes(
        selection[S::SECURE_INDEX_AFTER]
            .try_into()
            .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?,
    );
    let logical_indexes_valid = if operation == 0 {
        before == 0 && after == 0
    } else {
        before.checked_add(1) == Some(after)
    };
    if operation > 5
        || nonzero(S::CANDIDATE_ENVELOPE_DIGEST) != outgoing
        || nonzero(S::TERMINAL_BODY_COMMITMENT) != outgoing
        || !logical_indexes_valid
    {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KagemushaCoreCoordinatorSenderKindV1 {
    SendSplit,
    RedeemSplit,
}

fn require_sender_input_fields(
    fields: &[Vec<u8>],
    start: usize,
) -> Result<(KagemushaCoreCoordinatorSenderKindV1, usize), KagemushaCoreCoordinatorFrameErrorV1> {
    match require_u32_field(fields.get(start))? {
        KAGEMUSHA_CORE_COORDINATOR_SEND_SPLIT_V1 => {
            require_nonempty_field(fields.get(start + 1))?;
            Ok((KagemushaCoreCoordinatorSenderKindV1::SendSplit, start + 2))
        }
        KAGEMUSHA_CORE_COORDINATOR_REDEEM_SPLIT_V1 => {
            require_positive_u128_field(fields.get(start + 1))?;
            require_nonempty_field(fields.get(start + 2))?;
            Ok((KagemushaCoreCoordinatorSenderKindV1::RedeemSplit, start + 3))
        }
        _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
    }
}

fn require_qualification_fields(
    fields: &[Vec<u8>],
    start: usize,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    require_exact_u32_field(
        fields.get(start),
        KAGEMUSHA_CORE_COORDINATOR_PROTOCOL_VERSION_V1,
    )?;
    require_nonzero_digest_field(fields.get(start + 1))?;
    require_nonempty_field(fields.get(start + 2))?;
    require_nonempty_field(fields.get(start + 3))?;
    require_exact_u32_field(
        fields.get(start + 4),
        KAGEMUSHA_CORE_COORDINATOR_COMPLETE_CAPABILITY_MASK_V1,
    )
}

fn require_terminal_receipt_field(
    field: Option<&Vec<u8>>,
    sender_kind: KagemushaCoreCoordinatorSenderKindV1,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    let field = field.ok_or(KagemushaCoreCoordinatorFrameErrorV1::Field)?;
    let (tag, payload) = field
        .split_first_chunk::<4>()
        .ok_or(KagemushaCoreCoordinatorFrameErrorV1::Field)?;
    let expected = match sender_kind {
        KagemushaCoreCoordinatorSenderKindV1::SendSplit => KAGEMUSHA_CORE_COORDINATOR_SEND_SPLIT_V1,
        KagemushaCoreCoordinatorSenderKindV1::RedeemSplit => {
            KAGEMUSHA_CORE_COORDINATOR_REDEEM_SPLIT_V1
        }
    };
    if u32::from_le_bytes(*tag) != expected || payload.is_empty() {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    Ok(())
}

fn require_field_count(
    fields: &[Vec<u8>],
    expected: usize,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    if fields.len() != expected {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    Ok(())
}

fn require_nonempty_field(
    field: Option<&Vec<u8>>,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    if field.is_none_or(Vec::is_empty) {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    Ok(())
}

fn require_bounded_nonempty_field(
    field: Option<&Vec<u8>>,
    maximum: usize,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    let field = field.ok_or(KagemushaCoreCoordinatorFrameErrorV1::Field)?;
    if field.is_empty() || field.len() > maximum {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    Ok(())
}

fn require_exact_length_field(
    field: Option<&Vec<u8>>,
    length: usize,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    if field.is_none_or(|field| field.len() != length) {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    Ok(())
}

fn require_nonzero_ticket_field(
    field: Option<&Vec<u8>>,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    let field = field.ok_or(KagemushaCoreCoordinatorFrameErrorV1::Field)?;
    let bytes: [u8; 8] = field
        .as_slice()
        .try_into()
        .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
    if u64::from_le_bytes(bytes) == 0 {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    Ok(())
}

fn require_device_operation_field(
    field: Option<&Vec<u8>>,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    let operation = require_u32_field(field)?;
    let Ok(operation) = u8::try_from(operation) else {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    };
    if KagemushaDeviceLifecycleOperationV1::from_code(operation).is_none() {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    Ok(())
}

pub(crate) const fn is_observation_operation_v1(operation: u32) -> bool {
    matches!(operation, 1 | 13 | 18 | 21)
}

fn require_sender_kind_field(
    field: Option<&Vec<u8>>,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    match require_u32_field(field)? {
        KAGEMUSHA_CORE_COORDINATOR_SEND_SPLIT_V1 | KAGEMUSHA_CORE_COORDINATOR_REDEEM_SPLIT_V1 => {
            Ok(())
        }
        _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
    }
}

fn require_u32_field(field: Option<&Vec<u8>>) -> Result<u32, KagemushaCoreCoordinatorFrameErrorV1> {
    let field = field.ok_or(KagemushaCoreCoordinatorFrameErrorV1::Field)?;
    let bytes: [u8; 4] = field
        .as_slice()
        .try_into()
        .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
    Ok(u32::from_le_bytes(bytes))
}

fn require_exact_u32_field(
    field: Option<&Vec<u8>>,
    expected: u32,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    if require_u32_field(field)? != expected {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    Ok(())
}

fn require_positive_u128_field(
    field: Option<&Vec<u8>>,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    let field = field.ok_or(KagemushaCoreCoordinatorFrameErrorV1::Field)?;
    if field.len() != 16 || !field.iter().any(|byte| *byte != 0) {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    Ok(())
}

fn require_nonzero_digest_field(
    field: Option<&Vec<u8>>,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    let field = field.ok_or(KagemushaCoreCoordinatorFrameErrorV1::Field)?;
    if field.len() != 32 || !field.iter().any(|byte| *byte != 0) {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    Ok(())
}

fn require_device_signature_field(
    field: Option<&Vec<u8>>,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    let field = field.ok_or(KagemushaCoreCoordinatorFrameErrorV1::Field)?;
    KagemushaDeviceSignatureV1::from_raw_bytes(field)
        .map(|_| ())
        .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)
}

// This is data-only canonical framing. Genuine proof verification remains
// compulsory inside the concrete native incoming owner before retention/CAS.
fn require_canonical_paired_proof(
    bytes: &[u8],
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    use iroha_data_model::kagemusha::KagemushaPairedProofV1;
    if bytes.is_empty() || bytes.len() > 8192 {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    let proof: KagemushaPairedProofV1 =
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(8192))
            .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
    proof
        .validate_shape_for_semantic_digest(proof.semantic_digest)
        .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)
}

/// Encode qualified-provider response fields into the exact bounded frame.
pub fn kagemusha_core_coordinator_encode_response_v1(
    fields: &[Vec<u8>],
) -> Result<Vec<u8>, KagemushaCoreCoordinatorFrameErrorV1> {
    encode_frame(fields, KAGEMUSHA_CORE_COORDINATOR_MAX_RESPONSE_BYTES_V1)
}

/// Decode an exact bounded coordinator response frame.
pub fn kagemusha_core_coordinator_decode_response_v1(
    frame: &[u8],
) -> Result<Vec<Vec<u8>>, KagemushaCoreCoordinatorFrameErrorV1> {
    decode_frame(frame, KAGEMUSHA_CORE_COORDINATOR_MAX_RESPONSE_BYTES_V1)
}

/// Validate the exact transport response shape for one already validated method request.
///
/// The request is included so the boundary can reject operation-ID, terminal-ID,
/// and installed-envelope substitution before any backend output reaches C or JNI.
pub fn kagemusha_core_coordinator_validate_method_response_v1(
    method: KagemushaCoreCoordinatorMethodV1,
    request_frame: &[u8],
    response_frame: &[u8],
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    kagemusha_core_coordinator_validate_method_request_v1(method, request_frame)?;
    let request = kagemusha_core_coordinator_decode_request_v1(request_frame)?;
    let response = kagemusha_core_coordinator_decode_response_v1(response_frame)?;
    match method {
        KagemushaCoreCoordinatorMethodV1::PreparedAppOperationApproval
        | KagemushaCoreCoordinatorMethodV1::PreparedAppEnrollmentPossession
        | KagemushaCoreCoordinatorMethodV1::PreparedOrdinaryAppIdentity => {
            app_owned_identity_frames::validate_response(method, &request, &response)
        }
        KagemushaCoreCoordinatorMethodV1::AuthenticatedHardwarePolicy => {
            require_field_count(&response, 3)?;
            for field in &response {
                require_nonzero_digest_field(Some(field))?;
            }
            Ok(())
        }
        KagemushaCoreCoordinatorMethodV1::ReserveOperationId => {
            require_field_count(&response, 1)?;
            require_nonzero_digest_field(response.first())?;
            require_equal_fields(response.first(), request.get(1))
        }
        KagemushaCoreCoordinatorMethodV1::BeginObservation => {
            require_field_count(&response, 1)?;
            require_nonzero_digest_field(response.first())
        }
        KagemushaCoreCoordinatorMethodV1::ExportOutgoingStateProof => {
            require_field_count(&response, 3)?;
            require_equal_fields(response.first(), request.first())?;
            require_bounded_nonempty_field(
                response.get(1),
                iroha_core_zk::kagemusha_v1_state::KAGEMUSHA_OUTGOING_STATE_PUBLIC_INPUT_ARCHIVE_MAX_BYTES_V1,
            )?;
            require_bounded_nonempty_field(
                response.get(2),
                iroha_data_model::kagemusha::KAGEMUSHA_PAIRED_PROOF_MAX_BYTES_V1,
            )
        }
        KagemushaCoreCoordinatorMethodV1::AcceptQualification
        | KagemushaCoreCoordinatorMethodV1::AcceptAuthenticatedReply => {
            require_field_count(&response, 0)
        }
        KagemushaCoreCoordinatorMethodV1::BeginSenderTransition => {
            require_field_count(&response, 2)?;
            require_nonzero_digest_field(response.first())?;
            require_nonempty_field(response.get(1))?;
            require_equal_fields(response.first(), request.first())
        }
        KagemushaCoreCoordinatorMethodV1::ProvePreparedSenderTransition
        | KagemushaCoreCoordinatorMethodV1::BuildTerminalEnvelope
        | KagemushaCoreCoordinatorMethodV1::RecoverTerminalEnvelope => {
            require_field_count(&response, 1)?;
            require_nonempty_field(response.first())
        }
        KagemushaCoreCoordinatorMethodV1::PrepareIncomingFold => {
            require_field_count(&response, 10)?;
            for index in [0, 1, 3, 4, 6, 8] {
                require_nonzero_digest_field(response.get(index))?;
            }
            require_equal_fields(response.get(1), request.get(1))?;
            if response[7].len() != 16 || response[7].iter().all(|byte| *byte == 0) {
                return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
            }
            for (index, maximum) in [(2, 8192), (5, 32768), (9, 8192)] {
                require_nonempty_field(response.get(index))?;
                if response[index].len() > maximum {
                    return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
                }
            }
            require_canonical_paired_proof(&response[9])?;
            Ok(())
        }
        KagemushaCoreCoordinatorMethodV1::CompleteIncomingFold
        | KagemushaCoreCoordinatorMethodV1::StageIncomingOriginal => {
            require_field_count(&response, 1)?;
            require_nonzero_digest_field(response.first())?;
            let selector = if method == KagemushaCoreCoordinatorMethodV1::CompleteIncomingFold {
                0
            } else {
                1
            };
            require_equal_fields(response.first(), request.get(selector))
        }
        KagemushaCoreCoordinatorMethodV1::AcceptInstalledTerminal => {
            require_field_count(&response, 2)?;
            require_nonempty_field(response.first())?;
            require_nonempty_field(response.get(1))?;
            require_equal_fields(response.first(), request.get(1))
        }
        KagemushaCoreCoordinatorMethodV1::RecoverSender => {
            if response.is_empty() {
                return Ok(());
            }
            require_field_count(&response, 3)?;
            require_nonzero_digest_field(response.first())?;
            require_nonzero_digest_field(response.get(1))?;
            require_nonempty_field(response.get(2))?;
            match request.first().map(Vec::as_slice) {
                Some([KAGEMUSHA_CORE_COORDINATOR_RECOVER_BY_TERMINAL_ID_V1]) => {
                    require_equal_fields(response.get(1), request.get(1))
                }
                Some([KAGEMUSHA_CORE_COORDINATOR_RECOVER_BY_OPERATION_ID_V1]) => {
                    require_equal_fields(response.first(), request.get(1))
                }
                _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
            }
        }
        KagemushaCoreCoordinatorMethodV1::ReleaseOutbox => {
            require_field_count(&response, 5)?;
            require_nonzero_digest_field(response.first())?;
            require_nonempty_field(response.get(1))?;
            require_nonzero_digest_field(response.get(2))?;
            require_nonempty_field(response.get(3))?;
            require_nonempty_field(response.get(4))?;
            // Both sender kinds must release the exact installed envelope supplied by the
            // caller. The operation identity in response[0] is distinct from the requested
            // terminal identity, so only the envelope can be correlated at this frame layer.
            let (_, terminal_start) = require_sender_input_fields(&request, 1)?;
            require_equal_fields(response.get(3), request.get(terminal_start))
        }
        KagemushaCoreCoordinatorMethodV1::InitialEnrollment => {
            match require_u32_field(request.first())? {
                INITIAL_ENROLLMENT_BEGIN_V1 | INITIAL_ENROLLMENT_READ_SELECTION_V1 => {
                    require_field_count(&response, 7)?;
                    require_nonzero_ticket_field(response.first())?;
                    for index in 1..=5 {
                        require_nonzero_digest_field(response.get(index))?;
                    }
                    require_nonzero_ticket_field(response.get(6))
                }
                INITIAL_ENROLLMENT_VERIFY_APP_PREPARATION_V1 => {
                    require_field_count(&response, 1)?;
                    require_nonzero_digest_field(response.first())?;
                    if response[0].as_slice() != &request[2][49..81] {
                        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
                    }
                    Ok(())
                }
                9 => {
                    require_field_count(&response, 5)?;
                    require_nonzero_ticket_field(response.first())?;
                    require_bounded_nonempty_field(response.get(1), 16 * 1024)?;
                    require_nonzero_digest_field(response.get(2))?;
                    require_bounded_nonempty_field(response.get(3), 2 * 1024)?;
                    require_nonzero_digest_field(response.get(4))
                }
                10 => {
                    require_field_count(&response, 1)?;
                    require_equal_fields(response.first(), request.get(1))
                }
                11 => require_field_count(&response, 0),
                INITIAL_ENROLLMENT_ACCEPT_CHALLENGE_V1 => {
                    require_field_count(&response, 4)?;
                    require_equal_fields(response.first(), request.get(1))?;
                    require_equal_fields(response.get(1), request.get(7))?;
                    require_equal_fields(response.get(2), request.get(8))?;
                    require_equal_fields(response.get(3), request.get(9))
                }
                INITIAL_ENROLLMENT_PREPARE_PROOF_V1 | INITIAL_ENROLLMENT_READ_PROOF_V1 => {
                    require_field_count(&response, 3)?;
                    require_equal_fields(response.first(), request.get(1))?;
                    require_nonzero_digest_field(response.get(1))?;
                    require_bounded_nonempty_field(
                        response.get(2),
                        iroha_data_model::kagemusha::KAGEMUSHA_RETAIL_ENROLLMENT_PROOF_MAX_BYTES_V1,
                    )
                }
                INITIAL_ENROLLMENT_COMPLETE_V1 => {
                    require_field_count(&response, 2)?;
                    require_equal_fields(response.first(), request.get(1))?;
                    require_nonzero_digest_field(response.get(1))
                }
                INITIAL_ENROLLMENT_CANCEL_V1 => require_field_count(&response, 0),
                _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
            }
        }
        KagemushaCoreCoordinatorMethodV1::AcknowledgeCommittedAppAttest => {
            require_field_count(&response, 7)?;
            require_equal_fields(response.first(), request.first())?;
            for (response_index, request_index) in [(1, 1), (2, 2), (3, 3)] {
                let expected = Sha256::digest(&request[request_index]);
                if response[response_index].as_slice() != expected.as_slice() {
                    return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
                }
            }
            let actual =
                iroha_data_model::kagemusha::kagemusha_app_attest_original_counter_v1(&request[3])
                    .map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
            require_exact_u32_field(response.get(4), actual)?;
            require_equal_fields(response.get(5), request.get(5))?;
            require_equal_fields(response.get(6), request.get(6))
        }
    }
}

fn require_equal_fields(
    left: Option<&Vec<u8>>,
    right: Option<&Vec<u8>>,
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    match (left, right) {
        (Some(left), Some(right)) if left == right => Ok(()),
        _ => Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
    }
}

/// Validate a lexical absolute durable-store path without opening or creating storage.
///
/// The backend must still anchor every component to an app-private directory descriptor,
/// reject symlinks and hard-link aliases, and authenticate the opened durable store.
pub fn kagemusha_core_coordinator_validate_storage_path_v1(
    path: &[u8],
) -> Result<&str, KagemushaCoreCoordinatorFrameErrorV1> {
    if path.is_empty()
        || path.len() > KAGEMUSHA_CORE_COORDINATOR_MAX_STORAGE_PATH_BYTES_V1
        || path.contains(&0)
    {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Size);
    }
    let path =
        core::str::from_utf8(path).map_err(|_| KagemushaCoreCoordinatorFrameErrorV1::Field)?;
    if !path.starts_with('/')
        || path.len() == 1
        || path
            .bytes()
            .any(|byte| byte < 0x20 || byte == 0x7f || byte == b'\\')
        || path[1..]
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(KagemushaCoreCoordinatorFrameErrorV1::Field);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Clone, Copy)]
    enum TestResponse {
        ReserveValid,
        ReleaseValid,
        ReleaseSubstituted,
    }

    fn u32_field(value: u32) -> Vec<u8> {
        value.to_le_bytes().to_vec()
    }

    fn digest(byte: u8) -> Vec<u8> {
        vec![byte; 32]
    }

    fn qualification_fields() -> Vec<Vec<u8>> {
        vec![
            u32_field(KAGEMUSHA_CORE_COORDINATOR_PROTOCOL_VERSION_V1),
            digest(0x31),
            b"hardware-profile".to_vec(),
            b"hardware-credential".to_vec(),
            u32_field(KAGEMUSHA_CORE_COORDINATOR_COMPLETE_CAPABILITY_MASK_V1),
        ]
    }

    fn terminal_receipt_field(sender_kind: u32) -> Vec<u8> {
        let mut receipt = u32_field(sender_kind);
        receipt.extend_from_slice(b"canonical-terminal-receipt");
        receipt
    }

    fn append_fields(mut prefix: Vec<Vec<u8>>, suffix: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
        prefix.extend(suffix);
        prefix
    }

    fn send_begin_request_fields(operation_id: Vec<u8>) -> Vec<Vec<u8>> {
        append_fields(
            vec![
                operation_id,
                u32_field(KAGEMUSHA_CORE_COORDINATOR_SEND_SPLIT_V1),
                b"payment-request".to_vec(),
            ],
            qualification_fields(),
        )
    }

    fn redeem_begin_request_fields(operation_id: Vec<u8>) -> Vec<Vec<u8>> {
        append_fields(
            vec![
                operation_id,
                u32_field(KAGEMUSHA_CORE_COORDINATOR_REDEEM_SPLIT_V1),
                vec![1; 16],
                b"beneficiary".to_vec(),
            ],
            qualification_fields(),
        )
    }

    fn send_release_request_fields() -> Vec<Vec<u8>> {
        append_fields(
            vec![
                digest(0x41),
                u32_field(KAGEMUSHA_CORE_COORDINATOR_SEND_SPLIT_V1),
                b"payment-request".to_vec(),
                b"canonical-payment".to_vec(),
                terminal_receipt_field(KAGEMUSHA_CORE_COORDINATOR_SEND_SPLIT_V1),
            ],
            qualification_fields(),
        )
    }

    fn redeem_release_request_fields() -> Vec<Vec<u8>> {
        append_fields(
            vec![
                digest(0x42),
                u32_field(KAGEMUSHA_CORE_COORDINATOR_REDEEM_SPLIT_V1),
                vec![1; 16],
                b"beneficiary".to_vec(),
                b"canonical-redemption".to_vec(),
                terminal_receipt_field(KAGEMUSHA_CORE_COORDINATOR_REDEEM_SPLIT_V1),
            ],
            qualification_fields(),
        )
    }

    // Structural codec specimen only. Native monetary authority still requires real recursive
    // verification; a canonical archive must never be confused with an accepted proof.
    fn mobile_paired_proof_archive() -> Vec<u8> {
        use iroha_data_model::kagemusha::{
            KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1, KAGEMUSHA_WIRE_VERSION_V1,
            KagemushaPairedProofV1,
        };
        let proof = KagemushaPairedProofV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            eq_protocol_digest: [0x41; 32],
            ep_protocol_digest: [0x42; 32],
            semantic_digest: [0x43; 32],
            guard_eq_credential_audit: [0x44; 32],
            guard_ep_credential_audit: [0x45; 32],
            eq_deferred_audit: [0x46; 32],
            ep_deferred_audit: [0x47; 32],
            eq_proof: vec![0x48; 16],
            ep_proof: vec![0x49; 16],
            eq_history: vec![0x4a; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
            ep_history: vec![0x4b; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
        };
        proof
            .validate_shape_for_semantic_digest(proof.semantic_digest)
            .expect("canonical structural paired proof");
        norito::encode_canonical(&proof).expect("model-owned paired proof archive")
    }

    #[test]
    fn mobile_paired_proof_specimen_uses_exact_model_archive_and_shape() {
        use iroha_data_model::kagemusha::KagemushaPairedProofV1;
        let bytes = mobile_paired_proof_archive();
        assert_eq!(require_canonical_paired_proof(&bytes), Ok(()));
        assert!(require_canonical_paired_proof(b"unqualified-structural-pair").is_err());
        let proof = norito::decode_from_bytes::<KagemushaPairedProofV1>(&bytes)
            .expect("exact canonical paired proof archive");
        proof
            .validate_shape_for_semantic_digest([0x43; 32])
            .expect("fixed structural specimen");
        assert_eq!(norito::encode_canonical(&proof).unwrap(), bytes);
        assert!(
            proof
                .validate_shape_for_semantic_digest([0x50; 32])
                .is_err()
        );
        assert!(
            norito::decode_from_bytes::<KagemushaPairedProofV1>(b"unqualified-structural-pair")
                .is_err()
        );
        let mut tail = bytes;
        tail.push(0);
        assert!(require_canonical_paired_proof(&tail).is_err());
        assert!(norito::decode_from_bytes::<KagemushaPairedProofV1>(&tail).is_err());
    }

    fn mobile_request_cases() -> Vec<(KagemushaCoreCoordinatorMethodV1, &'static str, Vec<Vec<u8>>)>
    {
        let mut qualification = qualification_fields();
        qualification.push(digest(0x32));
        let authenticated_reply = append_fields(
            vec![
                u32_field(1),
                digest(0x33),
                b"canonical-command".to_vec(),
                b"canonical-reply".to_vec(),
                vec![1; 64],
            ],
            qualification_fields(),
        );
        let recovery = append_fields(
            vec![
                vec![KAGEMUSHA_CORE_COORDINATOR_RECOVER_BY_OPERATION_ID_V1],
                digest(0x34),
                u32_field(KAGEMUSHA_CORE_COORDINATOR_SEND_SPLIT_V1),
            ],
            qualification_fields(),
        );
        vec![
            (
                KagemushaCoreCoordinatorMethodV1::PreparedAppOperationApproval,
                "app-approval-recheck",
                vec![u32_field(6), 1u64.to_le_bytes().to_vec()],
            ),
            (
                KagemushaCoreCoordinatorMethodV1::PreparedAppEnrollmentPossession,
                "app-possession-recheck",
                vec![u32_field(6), 1u64.to_le_bytes().to_vec()],
            ),
            (
                KagemushaCoreCoordinatorMethodV1::PreparedOrdinaryAppIdentity,
                "ordinary-identity-recheck",
                vec![u32_field(8), 1u64.to_le_bytes().to_vec()],
            ),
            (
                KagemushaCoreCoordinatorMethodV1::ReserveOperationId,
                "reserve",
                vec![u32_field(22), digest(0x61), b"public-binding".to_vec()],
            ),
            (
                KagemushaCoreCoordinatorMethodV1::AcceptQualification,
                "qualification",
                qualification,
            ),
            (
                KagemushaCoreCoordinatorMethodV1::AcceptAuthenticatedReply,
                "authenticated-reply",
                authenticated_reply,
            ),
            (
                KagemushaCoreCoordinatorMethodV1::BeginSenderTransition,
                "begin-send",
                send_begin_request_fields(digest(0x35)),
            ),
            (
                KagemushaCoreCoordinatorMethodV1::BeginSenderTransition,
                "begin-redeem",
                redeem_begin_request_fields(digest(0x36)),
            ),
            (
                KagemushaCoreCoordinatorMethodV1::ProvePreparedSenderTransition,
                "prove",
                vec![
                    b"canonical-preparation".to_vec(),
                    b"authenticated-reply".to_vec(),
                ],
            ),
            (
                KagemushaCoreCoordinatorMethodV1::BuildTerminalEnvelope,
                "terminal-envelope",
                vec![
                    terminal_original_fixture_fields()[0].clone(),
                    terminal_original_fixture_fields()[1].clone(),
                ],
            ),
            (
                KagemushaCoreCoordinatorMethodV1::AcceptInstalledTerminal,
                "installed-terminal",
                vec![
                    b"canonical-candidate".to_vec(),
                    b"canonical-envelope".to_vec(),
                    b"install-reply".to_vec(),
                    b"installed-reply".to_vec(),
                    b"snapshot-reply".to_vec(),
                ],
            ),
            (
                KagemushaCoreCoordinatorMethodV1::RecoverSender,
                "recover-sender",
                recovery,
            ),
            (
                KagemushaCoreCoordinatorMethodV1::RecoverTerminalEnvelope,
                "recover-envelope",
                vec![
                    b"canonical-preparation".to_vec(),
                    b"installed-reply".to_vec(),
                ],
            ),
            (
                KagemushaCoreCoordinatorMethodV1::ReleaseOutbox,
                "release-send",
                send_release_request_fields(),
            ),
            (
                KagemushaCoreCoordinatorMethodV1::ReleaseOutbox,
                "release-redeem",
                redeem_release_request_fields(),
            ),
            (
                KagemushaCoreCoordinatorMethodV1::BeginObservation,
                "observation-credential",
                observation_request_fields(1),
            ),
            (
                KagemushaCoreCoordinatorMethodV1::BeginObservation,
                "observation-time",
                observation_request_fields(13),
            ),
            (
                KagemushaCoreCoordinatorMethodV1::BeginObservation,
                "observation-watermark",
                observation_request_fields(18),
            ),
            (
                KagemushaCoreCoordinatorMethodV1::BeginObservation,
                "observation-wallet",
                observation_request_fields(21),
            ),
            (
                KagemushaCoreCoordinatorMethodV1::InitialEnrollment,
                "initial-enrollment-begin",
                vec![
                    u32_field(INITIAL_ENROLLMENT_BEGIN_V1),
                    b"i105example".to_vec(),
                ],
            ),
            (
                KagemushaCoreCoordinatorMethodV1::AcknowledgeCommittedAppAttest,
                "app-attest-ack",
                app_attest_ack_request_fields(),
            ),
            (
                KagemushaCoreCoordinatorMethodV1::ExportOutgoingStateProof,
                "outgoing-state-proof-export",
                vec![digest(0x66)],
            ),
            (
                KagemushaCoreCoordinatorMethodV1::PrepareIncomingFold,
                "incoming-prepare",
                vec![u32_field(1), digest(0x76)],
            ),
            (
                KagemushaCoreCoordinatorMethodV1::CompleteIncomingFold,
                "incoming-complete",
                vec![
                    digest(0x71),
                    mobile_paired_proof_archive(),
                    b"unqualified-original-guard".to_vec(),
                    vec![1; 64],
                ],
            ),
            (
                KagemushaCoreCoordinatorMethodV1::StageIncomingOriginal,
                "incoming-stage",
                vec![u32_field(2), digest(0x76)],
            ),
            (
                KagemushaCoreCoordinatorMethodV1::AuthenticatedHardwarePolicy,
                "authenticated-hardware-policy",
                Vec::new(),
            ),
        ]
    }

    fn terminal_original_fixture_fields() -> Vec<Vec<u8>> {
        let archives: norito::json::Value = norito::json::from_str(include_str!(
            "../../../fixtures/offline/kagemusha_core_coordinator_archives_v1.json"
        ))
        .unwrap();
        let original: norito::json::Value = norito::json::from_str(include_str!(
            "../../../fixtures/offline/kagemusha_core_terminal_original_v1.json"
        ))
        .unwrap();
        vec![
            hex::decode(archives["candidate"]["norito_hex"].as_str().unwrap()).unwrap(),
            hex::decode(original["signedResponseHex"].as_str().unwrap()).unwrap(),
        ]
    }

    #[test]
    fn terminal_frame_requires_full_original_op7_success_and_exact_candidate_identity() {
        let valid = terminal_original_fixture_fields();
        let frame = kagemusha_core_coordinator_encode_request_v1(&valid).unwrap();
        kagemusha_core_coordinator_validate_method_request_v1(
            KagemushaCoreCoordinatorMethodV1::BuildTerminalEnvelope,
            &frame,
        )
        .unwrap();
        for mutation in 0..6 {
            let mut fields = valid.clone();
            match mutation {
                0 => fields[1] = b"inner-reply".to_vec(),
                1 => fields[1][10] = 8,
                2 => fields[1][11] = 1,
                3 => fields[1][12] ^= 1,
                4 => fields[1].push(0),
                5 => fields[0] = b"caller-candidate".to_vec(),
                _ => unreachable!(),
            }
            let frame = kagemusha_core_coordinator_encode_request_v1(&fields).unwrap();
            assert!(
                kagemusha_core_coordinator_validate_method_request_v1(
                    KagemushaCoreCoordinatorMethodV1::BuildTerminalEnvelope,
                    &frame
                )
                .is_err()
            );
        }
    }

    fn generated_terminal_original_fixture() -> norito::json::Value {
        use crate::kagemusha_device_bridge_v1::sender_payload::*;
        use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
        let archives: norito::json::Value = norito::json::from_str(include_str!(
            "../../../fixtures/offline/kagemusha_core_coordinator_archives_v1.json"
        ))
        .unwrap();
        let candidate = archives::KagemushaCoreSenderCandidateArchiveV1::decode_canonical_exact(
            &hex::decode(archives["candidate"]["norito_hex"].as_str().unwrap()).unwrap(),
        )
        .unwrap();
        let command = SenderCommandV1 {
            version: 1,
            operation: 7,
            operation_id: candidate.preparation.operation_id,
            context: candidate.preparation.context.clone(),
            body: SenderCommandBodyV1::Commit {
                selector: candidate.selector,
                candidate_digest: candidate.candidate_digest,
                hardware_authorization: candidate.hardware_commit_authorization.clone(),
            },
        };
        let prepare = SenderCommandV1::decode_canonical_exact(
            5,
            [7; 32],
            &canonical_command_body_for_tests(5).unwrap(),
        )
        .unwrap();
        let SenderCommandBodyV1::Prepare { inputs } = prepare.body else {
            panic!("fixture preparation")
        };
        let authorization = SenderHardwareAuthorizationV1::decode_canonical_exact(
            &candidate.hardware_commit_authorization,
        )
        .unwrap();
        let reply = SenderReplyV1 {
            version: 1,
            operation: 7,
            request_id: command.operation_id,
            context: command.context.clone(),
            index_revision: 1,
            body: SenderReplyBodyV1::Lookup(Some(SenderRecoveryItemV1 {
                record: SenderRecordV1 {
                    operation_id: command.operation_id,
                    context: command.context.clone(),
                    inputs_digest: candidate.preparation.inputs_digest,
                    operation_kind: inputs.operation_kind(),
                    preparation_id: candidate.selector.preparation_id,
                    outbox_reservation_id: [0x71; 32],
                    outcome_id: authorization.outcome_id,
                    phase: SenderPhaseV1::Committed,
                    record_revision: 1,
                    inputs: Some(inputs),
                    candidate_digest: Some(candidate.candidate_digest),
                    commit_certificate_digest: Some([0x72; 32]),
                    envelope_digest: None,
                    terminal_receipt_digest: None,
                },
                canonical_envelope: Vec::new(),
            })),
        };
        let command_bytes = command.encode_canonical().unwrap();
        let payload = reply.encode_canonical(&command, &command.context).unwrap();
        let policy = [0x62; 32];
        let report = [0x63; 32];
        let transcript = iroha_data_model::kagemusha::kagemusha_device_response_signing_bytes_v1(
            7,
            command.operation_id,
            &command_bytes,
            &payload,
            policy,
            report,
        )
        .unwrap();
        let key = SigningKey::from_bytes((&[0x64; 32]).into()).unwrap();
        let signature: Signature = key.sign(&transcript);
        let signature = signature.normalize_s().unwrap_or(signature).to_bytes();
        let mut frame = b"IKGMJRS1".to_vec();
        frame.extend_from_slice(&1_u16.to_le_bytes());
        frame.extend_from_slice(&[7, 0]);
        frame.extend_from_slice(&command.operation_id);
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        frame.extend_from_slice(&64_u32.to_le_bytes());
        frame.extend_from_slice(&Sha256::digest(&payload));
        frame.extend_from_slice(&Sha256::digest(signature));
        frame.extend_from_slice(&payload);
        frame.extend_from_slice(&signature);
        let mut value = norito::json::Map::new();
        for (k, v) in [("scope", "Known diagnostic P256 key; public signed frame only. Certificate digest and candidate are synthetic; no monetary or release authority.".to_owned()), ("signedResponseHex", hex::encode(frame)), ("canonicalCommandHex", hex::encode(command_bytes)), ("devicePublicKeyHex", hex::encode(key.verifying_key().to_encoded_point(false).as_bytes())), ("hardwarePolicyDigestHex", hex::encode(policy)), ("qualificationReportDigestHex", hex::encode(report))] { value.insert(k.to_owned(), norito::json::Value::String(v)); }
        norito::json::Value::Object(value)
    }

    #[test]
    fn diagnostic_terminal_original_fixture_matches_native_canonical_codec() {
        let generated = generated_terminal_original_fixture();
        if let Some(output) = std::env::var_os("IROHA_UPDATE_KAGEMUSHA_TERMINAL_FIXTURE") {
            // Developer-only candidate emission never replaces checked-in fixture bytes.
            let output = std::path::PathBuf::from(output);
            let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap();
            assert!(
                output.is_absolute(),
                "candidate output directory must be absolute"
            );
            assert_eq!(
                output.canonicalize().unwrap(),
                output,
                "candidate output directory must be canonical"
            );
            assert!(
                output.starts_with(repository.join("dist")),
                "candidate output directory must be under ignored dist"
            );
            assert!(
                output.is_dir(),
                "candidate output must be an existing directory"
            );
            std::fs::write(
                output.join("kagemusha_core_terminal_original_v1.json"),
                norito::json::to_json_pretty(&generated).unwrap() + "\n",
            )
            .unwrap();
            std::fs::write(
                output.join("kagemusha_core_coordinator_frame_v1.tsv"),
                canonical_shared_sdk_frames(&generated),
            )
            .unwrap();
        } else {
            let fixture: norito::json::Value = norito::json::from_str(include_str!(
                "../../../fixtures/offline/kagemusha_core_terminal_original_v1.json"
            ))
            .unwrap();
            assert_eq!(fixture, generated);
        }
    }

    #[test]
    fn original_terminal_fixture_has_real_device_signature_but_no_monetary_authority() {
        let fixture: norito::json::Value = norito::json::from_str(include_str!(
            "../../../fixtures/offline/kagemusha_core_terminal_original_v1.json"
        ))
        .unwrap();
        let bytes = |key: &str| hex::decode(fixture[key].as_str().unwrap()).unwrap();
        let response = bytes("signedResponseHex");
        let command = bytes("canonicalCommandHex");
        let key = iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1::from_sec1_bytes(&bytes(
            "devicePublicKeyHex",
        ))
        .unwrap();
        let policy: [u8; 32] = bytes("hardwarePolicyDigestHex").try_into().unwrap();
        let report: [u8; 32] = bytes("qualificationReportDigestHex").try_into().unwrap();
        iroha_data_model::kagemusha::kagemusha_verify_device_response_v1(
            &response, &command, 7, [7; 32], policy, report, &key,
        )
        .unwrap();
        let mut changed = response;
        let last = changed.len() - 1;
        changed[last] ^= 1;
        assert!(
            iroha_data_model::kagemusha::kagemusha_verify_device_response_v1(
                &changed, &command, 7, [7; 32], policy, report, &key
            )
            .is_err()
        );
    }

    fn app_attest_ack_request_fields() -> Vec<Vec<u8>> {
        vec![
            digest(0x11),
            b"app-attest-key".to_vec(),
            app_attest_selection_for_tests(),
            app_attest_assertion_for_tests(11),
            u32_field(4),
            digest(0x33),
            digest(0x44),
        ]
    }

    fn app_attest_selection_for_tests() -> Vec<u8> {
        use KagemushaHardwareSelectionSigningLayoutV1 as S;

        let mut selection = APP_ATTEST_SELECTION_SIGNING_DOMAIN_V1.to_vec();
        selection.extend_from_slice(&APP_ATTEST_SELECTION_BODY_BYTES_V1.to_le_bytes());
        selection.extend_from_slice(&[0x42; APP_ATTEST_SELECTION_BODY_BYTES_V1 as usize]);
        selection[S::VERSION].copy_from_slice(&1_u16.to_le_bytes());
        selection[S::POLICY_EPOCH].copy_from_slice(&1_u64.to_le_bytes());
        selection[S::HARDWARE_EPOCH_GENERATION].copy_from_slice(&1_u64.to_le_bytes());
        selection[S::OPERATION_TAG.start] = 2;
        selection[S::SECURE_INDEX_BEFORE].copy_from_slice(&70_u128.to_le_bytes());
        selection[S::SECURE_INDEX_AFTER].copy_from_slice(&71_u128.to_le_bytes());
        selection
    }

    fn app_attest_assertion_for_tests(counter: u32) -> Vec<u8> {
        // Canonical original shape only: this inert DER is never authenticated.
        let mut raw = vec![0xa2, 0x71];
        raw.extend_from_slice(b"authenticatorData");
        raw.extend_from_slice(&[0x58, 37]);
        raw.extend_from_slice(&[0x42; 32]);
        raw.push(0x40);
        raw.extend_from_slice(&counter.to_be_bytes());
        raw.push(0x69);
        raw.extend_from_slice(b"signature");
        raw.push(0x48);
        raw.extend_from_slice(&[0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01]);
        raw
    }

    fn observation_request_fields(operation: u8) -> Vec<Vec<u8>> {
        let command = crate::kagemusha_device_bridge_v1::canonical_stock_command_for_tests(
            KagemushaDeviceLifecycleOperationV1::from_code(operation).unwrap(),
        )
        .unwrap();
        vec![
            u32_field(u32::from(operation)),
            command[crate::kagemusha_device_bridge_v1::COMMAND_HEADER_BYTES_V1..].to_vec(),
        ]
    }

    fn mobile_response_fields(
        method: KagemushaCoreCoordinatorMethodV1,
        request: &[Vec<u8>],
    ) -> Vec<Vec<u8>> {
        match method {
            KagemushaCoreCoordinatorMethodV1::PreparedAppOperationApproval
            | KagemushaCoreCoordinatorMethodV1::PreparedAppEnrollmentPossession
            | KagemushaCoreCoordinatorMethodV1::PreparedOrdinaryAppIdentity => {
                vec![digest(0x7A), digest(0x7B)]
            }
            KagemushaCoreCoordinatorMethodV1::AuthenticatedHardwarePolicy => {
                vec![vec![1; 32], vec![2; 32], vec![3; 32]]
            }
            KagemushaCoreCoordinatorMethodV1::PrepareIncomingFold => vec![
                digest(0x71),
                request[1].clone(),
                b"public-hardware-statement".to_vec(),
                digest(0x72),
                digest(0x73),
                b"original-root-signing-bytes".to_vec(),
                digest(0x74),
                1_u128.to_le_bytes().to_vec(),
                digest(0x75),
                mobile_paired_proof_archive(),
            ],
            KagemushaCoreCoordinatorMethodV1::CompleteIncomingFold => vec![request[0].clone()],
            KagemushaCoreCoordinatorMethodV1::StageIncomingOriginal => vec![request[1].clone()],
            KagemushaCoreCoordinatorMethodV1::ReserveOperationId => vec![request[1].clone()],
            KagemushaCoreCoordinatorMethodV1::BeginObservation => vec![digest(0x65)],
            KagemushaCoreCoordinatorMethodV1::ExportOutgoingStateProof => vec![
                request[0].clone(),
                b"public-state-inputs".to_vec(),
                b"original-paired-proof".to_vec(),
            ],
            KagemushaCoreCoordinatorMethodV1::AcceptQualification
            | KagemushaCoreCoordinatorMethodV1::AcceptAuthenticatedReply => Vec::new(),
            KagemushaCoreCoordinatorMethodV1::BeginSenderTransition => {
                vec![request[0].clone(), b"canonical-preparation".to_vec()]
            }
            KagemushaCoreCoordinatorMethodV1::ProvePreparedSenderTransition => {
                vec![b"canonical-candidate".to_vec()]
            }
            KagemushaCoreCoordinatorMethodV1::BuildTerminalEnvelope
            | KagemushaCoreCoordinatorMethodV1::RecoverTerminalEnvelope => {
                vec![b"canonical-envelope".to_vec()]
            }
            KagemushaCoreCoordinatorMethodV1::AcceptInstalledTerminal => {
                vec![request[1].clone(), b"aggregate-state".to_vec()]
            }
            KagemushaCoreCoordinatorMethodV1::RecoverSender => {
                vec![
                    request[1].clone(),
                    digest(0x62),
                    b"canonical-preparation".to_vec(),
                ]
            }
            KagemushaCoreCoordinatorMethodV1::ReleaseOutbox => {
                let (_, terminal_start) =
                    require_sender_input_fields(request, 1).expect("valid sender request");
                vec![
                    digest(0x63),
                    b"canonical-preparation".to_vec(),
                    digest(0x64),
                    request[terminal_start].clone(),
                    b"hardware-release-authorization".to_vec(),
                ]
            }
            KagemushaCoreCoordinatorMethodV1::InitialEnrollment => vec![
                7_u64.to_le_bytes().to_vec(),
                digest(0x44),
                digest(0x45),
                digest(0x46),
                digest(0x47),
                digest(0x48),
                120_007_u64.to_le_bytes().to_vec(),
            ],
            KagemushaCoreCoordinatorMethodV1::AcknowledgeCommittedAppAttest => vec![
                request[0].clone(),
                Sha256::digest(&request[1]).to_vec(),
                Sha256::digest(&request[2]).to_vec(),
                Sha256::digest(&request[3]).to_vec(),
                u32_field(11),
                request[5].clone(),
                request[6].clone(),
            ],
        }
    }

    fn canonical_shared_sdk_frames(terminal: &norito::json::Value) -> String {
        let cases = mobile_request_cases();
        let mut rows: Vec<_> = cases
            .iter()
            .map(|(method, name, request)| {
                (
                    *method,
                    *name,
                    request.clone(),
                    mobile_response_fields(*method, request),
                )
            })
            .collect();
        let (_, _, recovery) = cases
            .iter()
            .find(|(_, name, _)| *name == "recover-sender")
            .unwrap();
        rows.push((
            KagemushaCoreCoordinatorMethodV1::RecoverSender,
            "recover-missing",
            recovery.clone(),
            Vec::new(),
        ));
        let mut terminal_recovery = recovery.clone();
        terminal_recovery[0] = vec![KAGEMUSHA_CORE_COORDINATOR_RECOVER_BY_TERMINAL_ID_V1];
        terminal_recovery[2] = u32_field(KAGEMUSHA_CORE_COORDINATOR_REDEEM_SPLIT_V1);
        let response = vec![
            digest(0x61),
            terminal_recovery[1].clone(),
            b"canonical-preparation".to_vec(),
        ];
        rows.push((
            KagemushaCoreCoordinatorMethodV1::RecoverSender,
            "recover-terminal",
            terminal_recovery,
            response,
        ));
        assert_eq!(rows.len(), 28);
        let mut rendered = String::from(
            "# Structural native schema-2 frame vectors; opaque archive strings are not monetary proofs or qualification.\n# name\tmethod\trequest_hex\tresponse_hex\n",
        );
        for (method, name, mut request, response) in rows {
            if method == KagemushaCoreCoordinatorMethodV1::BuildTerminalEnvelope {
                request[1] = hex::decode(terminal["signedResponseHex"].as_str().unwrap()).unwrap();
            }
            let request = kagemusha_core_coordinator_encode_request_v1(&request).unwrap();
            let response = kagemusha_core_coordinator_encode_response_v1(&response).unwrap();
            kagemusha_core_coordinator_validate_method_response_v1(method, &request, &response)
                .unwrap_or_else(|error| panic!("native fixture {name}: {error:?}"));
            rendered.push_str(&format!(
                "{name}\t{}\t{}\t{}\n",
                method.code(),
                hex::encode(request),
                hex::encode(response)
            ));
        }
        rendered
    }

    #[test]
    fn shared_sdk_frames_match_every_native_method_and_recovery_selector() {
        let generated = canonical_shared_sdk_frames(&generated_terminal_original_fixture());
        let fixture =
            include_str!("../../../fixtures/offline/kagemusha_core_coordinator_frame_v1.tsv");
        assert_eq!(
            fixture, generated,
            "shared fixture must match the complete native producer"
        );
        let mut fixtures = std::collections::BTreeMap::new();
        for line in fixture
            .lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
        {
            let columns: Vec<_> = line.split('\t').collect();
            assert_eq!(columns.len(), 4, "invalid fixture row");
            let method = KagemushaCoreCoordinatorMethodV1::from_code(
                columns[1].parse().expect("method code"),
            )
            .expect("closed method");
            let request = hex::decode(columns[2]).expect("request hex");
            let response = hex::decode(columns[3]).expect("response hex");
            kagemusha_core_coordinator_validate_method_response_v1(method, &request, &response)
                .expect("native request/response correlation");
            assert!(
                fixtures
                    .insert(columns[0], (method, request, response))
                    .is_none()
            );
        }
        assert_eq!(fixtures.len(), 28);
        for (method, name, request_fields) in mobile_request_cases() {
            let (actual_method, request, response) =
                fixtures.get(name).expect("shared method case");
            assert_eq!(*actual_method, method);
            assert_eq!(
                *request,
                kagemusha_core_coordinator_encode_request_v1(&request_fields)
                    .expect("native request"),
                "{name}",
            );
            assert_eq!(
                *response,
                kagemusha_core_coordinator_encode_response_v1(&mobile_response_fields(
                    method,
                    &request_fields
                ))
                .expect("native response"),
                "{name}",
            );
        }
        let (_, missing_request, missing_response) = fixtures.get("recover-missing").unwrap();
        let missing_fields = kagemusha_core_coordinator_decode_request_v1(missing_request).unwrap();
        assert_eq!(
            missing_fields[0],
            [KAGEMUSHA_CORE_COORDINATOR_RECOVER_BY_OPERATION_ID_V1]
        );
        assert!(
            kagemusha_core_coordinator_decode_response_v1(missing_response)
                .unwrap()
                .is_empty()
        );
        let (_, terminal_request, _) = fixtures.get("recover-terminal").unwrap();
        let terminal_fields =
            kagemusha_core_coordinator_decode_request_v1(terminal_request).unwrap();
        assert_eq!(
            terminal_fields[0],
            [KAGEMUSHA_CORE_COORDINATOR_RECOVER_BY_TERMINAL_ID_V1]
        );
    }

    #[test]
    fn incoming_and_catalog_frames_reject_selector_substitution_and_incomplete_originals() {
        let cases = mobile_request_cases();
        for (method, _, fields) in cases
            .into_iter()
            .filter(|(method, _, _)| method.code() >= 15)
        {
            let request = kagemusha_core_coordinator_encode_request_v1(&fields).unwrap();
            let response = mobile_response_fields(method, &fields);
            let frame = kagemusha_core_coordinator_encode_response_v1(&response).unwrap();
            kagemusha_core_coordinator_validate_method_response_v1(method, &request, &frame)
                .unwrap();
            let mut extra = fields.clone();
            extra.push(vec![1]);
            assert!(
                kagemusha_core_coordinator_validate_method_request_v1(
                    method,
                    &kagemusha_core_coordinator_encode_request_v1(&extra).unwrap()
                )
                .is_err()
            );
            for index in 0..response.len() {
                let mut changed = response.clone();
                changed[index] = Vec::new();
                assert!(
                    kagemusha_core_coordinator_validate_method_response_v1(
                        method,
                        &request,
                        &kagemusha_core_coordinator_encode_response_v1(&changed).unwrap()
                    )
                    .is_err()
                );
            }
            if method == KagemushaCoreCoordinatorMethodV1::PrepareIncomingFold {
                for index in [0, 1, 3, 4, 6, 8] {
                    let mut changed = response.clone();
                    changed[index] = vec![0; 32];
                    assert!(
                        kagemusha_core_coordinator_validate_method_response_v1(
                            method,
                            &request,
                            &kagemusha_core_coordinator_encode_response_v1(&changed).unwrap()
                        )
                        .is_err()
                    );
                }
                let mut changed = response.clone();
                changed[1] = vec![9; 32];
                assert!(
                    kagemusha_core_coordinator_validate_method_response_v1(
                        method,
                        &request,
                        &kagemusha_core_coordinator_encode_response_v1(&changed).unwrap()
                    )
                    .is_err()
                );
                let mut changed = response.clone();
                changed[7] = 0_u128.to_le_bytes().to_vec();
                assert!(
                    kagemusha_core_coordinator_validate_method_response_v1(
                        method,
                        &request,
                        &kagemusha_core_coordinator_encode_response_v1(&changed).unwrap()
                    )
                    .is_err()
                );
            }
        }
    }

    fn canonical_sender_reservation_binding() -> Vec<u8> {
        let fixture: norito::json::Value = norito::json::from_str(include_str!(
            "../../../fixtures/offline/kagemusha_sender_reservation_v1.json"
        ))
        .unwrap();
        hex::decode(fixture["redeem_binding_hex"].as_str().unwrap()).unwrap()
    }

    struct TestBackend {
        invokes: AtomicUsize,
        closes: AtomicUsize,
        caller_path_pointer: AtomicUsize,
        caller_request_pointer: AtomicUsize,
        response: Mutex<TestResponse>,
    }

    impl KagemushaCoreCoordinatorBackendV1 for TestBackend {
        fn open(&self, storage_path: &str) -> Result<u64, KagemushaCoreCoordinatorBackendErrorV1> {
            assert_eq!(storage_path, "/durable/kagemusha.db");
            let caller_pointer = self.caller_path_pointer.swap(0, Ordering::SeqCst);
            if caller_pointer != 0 {
                assert_ne!(storage_path.as_ptr() as usize, caller_pointer);
            }
            Ok(7)
        }

        fn invoke(
            &self,
            handle: u64,
            method: KagemushaCoreCoordinatorMethodV1,
            request_frame: &[u8],
        ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1> {
            assert_eq!(handle, 7);
            let caller_pointer = self.caller_request_pointer.swap(0, Ordering::SeqCst);
            if caller_pointer != 0 {
                assert_ne!(request_frame.as_ptr() as usize, caller_pointer);
            }
            self.invokes.fetch_add(1, Ordering::SeqCst);
            match *self.response.lock().expect("response mode") {
                TestResponse::ReserveValid => {
                    assert_eq!(method, KagemushaCoreCoordinatorMethodV1::ReserveOperationId);
                    assert_eq!(
                        kagemusha_core_coordinator_decode_request_v1(request_frame),
                        Ok(vec![
                            u32_field(5),
                            digest(0x51),
                            canonical_sender_reservation_binding()
                        ])
                    );
                    kagemusha_core_coordinator_encode_response_v1(&[digest(0x51)])
                        .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)
                }
                mode @ (TestResponse::ReleaseValid | TestResponse::ReleaseSubstituted) => {
                    assert_eq!(method, KagemushaCoreCoordinatorMethodV1::ReleaseOutbox);
                    assert_eq!(
                        kagemusha_core_coordinator_validate_method_request_v1(
                            method,
                            request_frame
                        ),
                        Ok(())
                    );
                    let (expected_request, mut fields) = archive_boundary::tests::release_fields();
                    assert_eq!(
                        kagemusha_core_coordinator_decode_request_v1(request_frame).unwrap(),
                        expected_request
                    );
                    if matches!(mode, TestResponse::ReleaseSubstituted) {
                        fields[3] = b"another-installed-envelope".to_vec();
                    }
                    kagemusha_core_coordinator_encode_response_v1(&fields)
                        .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)
                }
            }
        }

        fn close(&self, handle: u64) -> Result<(), KagemushaCoreCoordinatorBackendErrorV1> {
            assert_eq!(handle, 7);
            self.closes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn coordinator_contract_and_methods_are_exact() {
        assert_eq!(
            KAGEMUSHA_CORE_COORDINATOR_CONTRACT_WORDS_V1,
            [2, 25, 3, 6, 54, 8, 7, 22, 16, 0xffff, 1, 21]
        );
        assert_eq!(
            KagemushaCoreCoordinatorMethodV1::ALL.map(KagemushaCoreCoordinatorMethodV1::code),
            [
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21
            ]
        );
        for method in KagemushaCoreCoordinatorMethodV1::ALL {
            assert_eq!(
                KagemushaCoreCoordinatorMethodV1::from_code(method.code()),
                Some(method)
            );
        }
        for unknown in [0, 22, u8::MAX] {
            assert_eq!(KagemushaCoreCoordinatorMethodV1::from_code(unknown), None);
        }
    }

    #[test]
    fn app_attest_commit_ack_correlates_original_counter_separate_from_financial_indexes() {
        let method = KagemushaCoreCoordinatorMethodV1::AcknowledgeCommittedAppAttest;
        let request = app_attest_ack_request_fields();
        let request_frame = kagemusha_core_coordinator_encode_request_v1(&request).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_request_v1(method, &request_frame),
            Ok(())
        );
        let response = vec![
            request[0].clone(),
            Sha256::digest(&request[1]).to_vec(),
            Sha256::digest(&request[2]).to_vec(),
            Sha256::digest(&request[3]).to_vec(),
            11_u32.to_le_bytes().to_vec(),
            request[5].clone(),
            request[6].clone(),
        ];
        let response_frame = kagemusha_core_coordinator_encode_response_v1(&response).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_response_v1(
                method,
                &request_frame,
                &response_frame,
            ),
            Ok(())
        );
        for index in 0..response.len() {
            let mut changed = response.clone();
            changed[index][0] ^= 1;
            let frame = kagemusha_core_coordinator_encode_response_v1(&changed).unwrap();
            assert!(
                kagemusha_core_coordinator_validate_method_response_v1(
                    method,
                    &request_frame,
                    &frame,
                )
                .is_err(),
                "substituted response field {index} passed"
            );
        }
        for index in [0, 1, 2, 3, 4, 5, 6] {
            let mut changed = request.clone();
            changed[index].clear();
            let frame = kagemusha_core_coordinator_encode_request_v1(&changed).unwrap();
            assert!(kagemusha_core_coordinator_validate_method_request_v1(method, &frame).is_err());
        }
        for index in [
            KagemushaHardwareSelectionSigningLayoutV1::VERSION.start,
            KagemushaHardwareSelectionSigningLayoutV1::OPERATION_TAG.start,
            KagemushaHardwareSelectionSigningLayoutV1::SECURE_INDEX_BEFORE.start,
            KagemushaHardwareSelectionSigningLayoutV1::SECURE_INDEX_AFTER.start,
        ] {
            let mut changed = request.clone();
            changed[2][index] ^= 1;
            let frame = kagemusha_core_coordinator_encode_request_v1(&changed).unwrap();
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(method, &frame),
                Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
                "signed Core S byte {index} was not bound"
            );
        }
        let mut lower_floor = request.clone();
        lower_floor[4] = 3_u32.to_le_bytes().to_vec();
        let frame = kagemusha_core_coordinator_encode_request_v1(&lower_floor).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_request_v1(method, &frame),
            Ok(())
        );
        for floor in [11_u32, 12] {
            let mut nonadvancing = request.clone();
            nonadvancing[4] = floor.to_le_bytes().to_vec();
            let frame = kagemusha_core_coordinator_encode_request_v1(&nonadvancing).unwrap();
            assert!(kagemusha_core_coordinator_validate_method_request_v1(method, &frame).is_err());
        }
        let mut skipped = request;
        skipped[4] = u32::MAX.to_le_bytes().to_vec();
        let frame = kagemusha_core_coordinator_encode_request_v1(&skipped).unwrap();
        assert!(kagemusha_core_coordinator_validate_method_request_v1(method, &frame).is_err());
    }

    #[test]
    fn initial_enrollment_selection_pins_nonzero_native_fields() {
        let method = KagemushaCoreCoordinatorMethodV1::InitialEnrollment;
        let request = kagemusha_core_coordinator_encode_request_v1(&[
            u32_field(INITIAL_ENROLLMENT_BEGIN_V1),
            b"i105example".to_vec(),
        ])
        .unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_request_v1(method, &request),
            Ok(())
        );
        let mut response = vec![7_u64.to_le_bytes().to_vec()];
        response.extend((1..=5).map(|byte| vec![byte; 32]));
        response.push(120_007_u64.to_le_bytes().to_vec());
        let good = kagemusha_core_coordinator_encode_response_v1(&response).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_response_v1(method, &request, &good),
            Ok(())
        );
        let read = kagemusha_core_coordinator_encode_request_v1(&[
            u32_field(INITIAL_ENROLLMENT_READ_SELECTION_V1),
            b"i105example".to_vec(),
        ])
        .unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_request_v1(method, &read),
            Ok(())
        );
        assert_eq!(
            kagemusha_core_coordinator_validate_method_response_v1(method, &read, &good),
            Ok(())
        );
        response[4] = vec![0; 32];
        let bad = kagemusha_core_coordinator_encode_response_v1(&response).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_response_v1(method, &request, &bad),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
        );
        let mut challenge = vec![
            u32_field(INITIAL_ENROLLMENT_ACCEPT_CHALLENGE_V1),
            7_u64.to_le_bytes().to_vec(),
            vec![1; 273],
            vec![2; 8],
            vec![3; 8],
            vec![4; 8],
            vec![5; 32],
            vec![6; 32],
            vec![7; 32],
            vec![8; 8],
            1_u64.to_le_bytes().to_vec(),
        ];
        let valid = kagemusha_core_coordinator_encode_request_v1(&challenge).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_request_v1(method, &valid),
            Ok(())
        );
        challenge[2].pop();
        let invalid = kagemusha_core_coordinator_encode_request_v1(&challenge).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_request_v1(method, &invalid),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
        );
    }

    #[test]
    fn initial_enrollment_phase_frames_bound_full_device_response_without_truncation() {
        let method = KagemushaCoreCoordinatorMethodV1::InitialEnrollment;
        let ticket = 7_u64.to_le_bytes().to_vec();
        let begin = kagemusha_core_coordinator_encode_request_v1(&[
            u32_field(INITIAL_ENROLLMENT_BEGIN_V1),
            b"i105example".to_vec(),
        ])
        .unwrap();
        let selected = vec![
            ticket.clone(),
            vec![0x44; 32],
            vec![0x45; 32],
            vec![0x46; 32],
            vec![0x47; 32],
            vec![0x48; 32],
            120_007_u64.to_le_bytes().to_vec(),
        ];
        let selected_frame = kagemusha_core_coordinator_encode_response_v1(&selected).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_response_v1(method, &begin, &selected_frame),
            Ok(()),
        );
        let old_five = kagemusha_core_coordinator_encode_response_v1(&selected[..5]).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_response_v1(method, &begin, &old_five),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
        );
        let mut no_deadline = selected;
        no_deadline[6] = vec![0; 8];
        let no_deadline = kagemusha_core_coordinator_encode_response_v1(&no_deadline).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_response_v1(method, &begin, &no_deadline),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
        );
        let complete_response_max =
            iroha_data_model::kagemusha::KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1;
        let request = kagemusha_core_coordinator_encode_request_v1(&[
            u32_field(INITIAL_ENROLLMENT_PREPARE_PROOF_V1),
            ticket.clone(),
            vec![0x51; 64],
            vec![0x52; complete_response_max],
        ])
        .unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_request_v1(method, &request),
            Ok(())
        );
        let oversized = kagemusha_core_coordinator_encode_request_v1(&[
            u32_field(INITIAL_ENROLLMENT_PREPARE_PROOF_V1),
            ticket.clone(),
            vec![0x51; 64],
            vec![0x52; complete_response_max + 1],
        ])
        .unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_request_v1(method, &oversized),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
        );
        let read = kagemusha_core_coordinator_encode_request_v1(&[
            u32_field(INITIAL_ENROLLMENT_READ_PROOF_V1),
            ticket,
        ])
        .unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_request_v1(method, &read),
            Ok(())
        );
    }

    #[test]
    fn coordinator_frames_roundtrip_and_reject_noncanonical_shapes() {
        let fields = vec![Vec::new(), vec![1, 2, 3], vec![0xa5; 64]];
        let request = kagemusha_core_coordinator_encode_request_v1(&fields).expect("request");
        assert_eq!(
            kagemusha_core_coordinator_decode_request_v1(&request),
            Ok(fields.clone())
        );
        let response = kagemusha_core_coordinator_encode_response_v1(&fields).expect("response");
        assert_eq!(
            kagemusha_core_coordinator_decode_response_v1(&response),
            Ok(fields)
        );

        let mut bad_magic = request.clone();
        bad_magic[0] ^= 1;
        assert_eq!(
            kagemusha_core_coordinator_decode_request_v1(&bad_magic),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Header)
        );
        let mut bad_version = request.clone();
        bad_version[8] = 1;
        assert_eq!(
            kagemusha_core_coordinator_decode_request_v1(&bad_version),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Header)
        );
        let mut reserved = request.clone();
        reserved[12] = 1;
        assert_eq!(
            kagemusha_core_coordinator_decode_request_v1(&reserved),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Header)
        );
        let mut trailing = request;
        trailing.push(0);
        assert_eq!(
            kagemusha_core_coordinator_decode_request_v1(&trailing),
            Err(KagemushaCoreCoordinatorFrameErrorV1::TrailingBytes)
        );
        assert_eq!(
            kagemusha_core_coordinator_encode_request_v1(&vec![Vec::new(); 17]),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Header)
        );
        assert_eq!(
            kagemusha_core_coordinator_encode_request_v1(&[vec![
                0;
                KAGEMUSHA_CORE_COORDINATOR_MAX_FIELD_BYTES_V1
                    + 1
            ]]),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
        );
    }

    #[test]
    fn outgoing_state_proof_export_binds_operation_and_rejects_untyped_archives() {
        let method = KagemushaCoreCoordinatorMethodV1::ExportOutgoingStateProof;
        let operation_id = digest(0x66);
        let request = kagemusha_core_coordinator_encode_request_v1(&[operation_id.clone()])
            .expect("bounded operation selector");
        kagemusha_core_coordinator_validate_method_request_v1(method, &request)
            .expect("exact selector");
        for rejected in [vec![0; 32], vec![0x66; 31]] {
            let invalid = kagemusha_core_coordinator_encode_request_v1(&[rejected]).unwrap();
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(method, &invalid),
                Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
            );
        }
        let response = kagemusha_core_coordinator_encode_response_v1(&[
            operation_id.clone(),
            b"not-canonical-public-inputs".to_vec(),
            b"not-canonical-proof".to_vec(),
        ])
        .unwrap();
        kagemusha_core_coordinator_validate_method_response_v1(method, &request, &response)
            .expect("transport shape only");
        assert_eq!(
            archive_boundary::validate_response(method, &request, &response),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
        );
        let substituted = kagemusha_core_coordinator_encode_response_v1(&[
            digest(0x67),
            b"not-canonical-public-inputs".to_vec(),
            b"not-canonical-proof".to_vec(),
        ])
        .unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_response_v1(method, &request, &substituted),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
        );
    }

    #[test]
    fn begin_and_recovery_frames_require_caller_persisted_operation_ids() {
        let operation_id = vec![0x5a; 32];
        let begin_fields = send_begin_request_fields(operation_id.clone());
        let begin =
            kagemusha_core_coordinator_encode_request_v1(&begin_fields).expect("begin frame");
        assert_eq!(
            kagemusha_core_coordinator_validate_method_request_v1(
                KagemushaCoreCoordinatorMethodV1::BeginSenderTransition,
                &begin,
            ),
            Ok(())
        );
        for bad_id in [Vec::new(), vec![0; 32], vec![1; 31]] {
            let frame =
                kagemusha_core_coordinator_encode_request_v1(&send_begin_request_fields(bad_id))
                    .expect("frame");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(
                    KagemushaCoreCoordinatorMethodV1::BeginSenderTransition,
                    &frame,
                ),
                Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
            );
        }
        let mut missing = begin_fields.clone();
        missing.pop();
        let mut trailing = begin_fields;
        trailing.push(b"trailing".to_vec());
        for wrong_shape in [missing, trailing] {
            let frame = kagemusha_core_coordinator_encode_request_v1(&wrong_shape).expect("frame");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(
                    KagemushaCoreCoordinatorMethodV1::BeginSenderTransition,
                    &frame,
                ),
                Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
            );
        }

        for selector in [
            KAGEMUSHA_CORE_COORDINATOR_RECOVER_BY_TERMINAL_ID_V1,
            KAGEMUSHA_CORE_COORDINATOR_RECOVER_BY_OPERATION_ID_V1,
        ] {
            let fields = append_fields(
                vec![
                    vec![selector],
                    operation_id.clone(),
                    u32_field(KAGEMUSHA_CORE_COORDINATOR_SEND_SPLIT_V1),
                ],
                qualification_fields(),
            );
            let frame =
                kagemusha_core_coordinator_encode_request_v1(&fields).expect("recovery frame");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(
                    KagemushaCoreCoordinatorMethodV1::RecoverSender,
                    &frame,
                ),
                Ok(())
            );
        }
        let invalid_selector = kagemusha_core_coordinator_encode_request_v1(&append_fields(
            vec![
                vec![2],
                operation_id,
                u32_field(KAGEMUSHA_CORE_COORDINATOR_SEND_SPLIT_V1),
            ],
            qualification_fields(),
        ))
        .expect("invalid selector frame");
        assert_eq!(
            kagemusha_core_coordinator_validate_method_request_v1(
                KagemushaCoreCoordinatorMethodV1::RecoverSender,
                &invalid_selector,
            ),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
        );
    }

    #[test]
    fn release_acceptance_requires_exact_original_frame_and_rejects_retired_layout() {
        // Public framing specimen only; no device or monetary authority is admitted.
        let payload = b"original-release-payload".to_vec();
        let mut signature = vec![0; 64];
        signature[31] = 1;
        signature[63] = 1;
        let id = [7; 32];
        let mut original = b"IKGMJRS1".to_vec();
        original.extend_from_slice(&1_u16.to_le_bytes());
        original.extend_from_slice(&[12, 0]);
        original.extend_from_slice(&id);
        original.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        original.extend_from_slice(&64_u32.to_le_bytes());
        original.extend_from_slice(&Sha256::digest(&payload));
        original.extend_from_slice(&Sha256::digest(&signature));
        original.extend_from_slice(&payload);
        original.extend_from_slice(&signature);
        let mut fields = append_fields(
            vec![
                u32_field(12),
                id.to_vec(),
                b"command".to_vec(),
                payload,
                signature,
            ],
            qualification_fields(),
        );
        let method = KagemushaCoreCoordinatorMethodV1::AcceptAuthenticatedReply;
        let check = |fields: &[Vec<u8>]| {
            let frame = kagemusha_core_coordinator_encode_request_v1(fields).unwrap();
            kagemusha_core_coordinator_validate_method_request_v1(method, &frame)
        };
        assert!(check(&fields).is_err());
        fields.push(original.clone());
        assert_eq!(check(&fields), Ok(()));
        for (index, replacement) in [
            (1, vec![8; 32]),
            (3, b"changed-reply".to_vec()),
            (4, {
                let mut value = fields[4].clone();
                value[31] = 2;
                value
            }),
            (10, original[..original.len() - 1].to_vec()),
        ] {
            let mut changed = fields.clone();
            changed[index] = replacement;
            assert!(check(&changed).is_err());
        }
        let mut changed = fields;
        changed[10][10] = 7;
        assert!(check(&changed).is_err());
    }

    #[test]
    fn signed_android_and_ios_requests_have_one_exact_method_matrix() {
        let expected_counts = [
            2, 2, 2, 3, 6, 10, 8, 9, 2, 2, 5, 8, 2, 10, 11, 2, 2, 2, 2, 2, 7, 1, 2, 4, 2, 0,
        ];
        let cases = mobile_request_cases();
        assert_eq!(
            cases
                .iter()
                .map(|(_, _, fields)| fields.len())
                .collect::<Vec<_>>(),
            expected_counts
        );
        for (method, label, fields) in cases {
            let frame = kagemusha_core_coordinator_encode_request_v1(&fields).expect("valid frame");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(method, &frame),
                Ok(()),
                "valid signed-app shape for {method:?}/{label}"
            );

            if !fields.is_empty() {
                let missing =
                    kagemusha_core_coordinator_encode_request_v1(&fields[..fields.len() - 1])
                        .expect("missing-field frame");
                assert_eq!(
                    kagemusha_core_coordinator_validate_method_request_v1(method, &missing),
                    Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
                    "missing field for {method:?}/{label}"
                );
            }

            let mut trailing_fields = fields;
            trailing_fields.push(b"trailing".to_vec());
            let trailing = kagemusha_core_coordinator_encode_request_v1(&trailing_fields)
                .expect("trailing-field frame");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(method, &trailing),
                Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
                "trailing field for {method:?}/{label}"
            );
        }
    }

    #[test]
    fn reservation_requires_persisted_caller_id_and_exact_echo() {
        let method = KagemushaCoreCoordinatorMethodV1::ReserveOperationId;
        let fields = vec![u32_field(5), digest(0x52), b"exact-public-binding".to_vec()];
        let request = kagemusha_core_coordinator_encode_request_v1(&fields).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_request_v1(method, &request),
            Ok(())
        );
        let correct = kagemusha_core_coordinator_encode_response_v1(&[digest(0x52)]).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_response_v1(method, &request, &correct),
            Ok(())
        );
        let substituted = kagemusha_core_coordinator_encode_response_v1(&[digest(0x53)]).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_response_v1(method, &request, &substituted),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
        );
        for invalid in [
            vec![u32_field(5), b"old-binding".to_vec()],
            vec![u32_field(5), vec![0; 32], b"binding".to_vec()],
            vec![u32_field(5), vec![1; 31], b"binding".to_vec()],
        ] {
            let frame = kagemusha_core_coordinator_encode_request_v1(&invalid).unwrap();
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(method, &frame),
                Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
            );
        }
        let mut retired = request;
        retired[8..10].copy_from_slice(&1_u16.to_le_bytes());
        assert_eq!(
            kagemusha_core_coordinator_decode_request_v1(&retired),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Header)
        );
    }

    #[test]
    fn observations_have_no_caller_nonce_and_require_one_nonzero_native_challenge() {
        let method = KagemushaCoreCoordinatorMethodV1::BeginObservation;
        for operation in [1, 13, 18, 21] {
            let fields = observation_request_fields(operation);
            let request = kagemusha_core_coordinator_encode_request_v1(&fields).unwrap();
            archive_boundary::validate_request(method, &request).unwrap();
            for nonce in [digest(0x65), digest(0x66)] {
                let response = kagemusha_core_coordinator_encode_response_v1(&[nonce]).unwrap();
                archive_boundary::validate_response(method, &request, &response).unwrap();
            }
            for invalid in [
                vec![],
                vec![vec![0; 32]],
                vec![vec![1; 31]],
                vec![digest(1), digest(2)],
            ] {
                let response = kagemusha_core_coordinator_encode_response_v1(&invalid).unwrap();
                assert!(archive_boundary::validate_response(method, &request, &response).is_err());
            }
            let old_reservation = kagemusha_core_coordinator_encode_request_v1(&[
                fields[0].clone(),
                digest(0x65),
                fields[1].clone(),
            ])
            .unwrap();
            assert!(archive_boundary::validate_request(method, &old_reservation).is_err());
            assert!(
                archive_boundary::validate_request(
                    KagemushaCoreCoordinatorMethodV1::ReserveOperationId,
                    &old_reservation,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn authenticated_reply_retains_exact_signature_and_rejects_retired_projection() {
        let (method, _, fields) = mobile_request_cases()
            .into_iter()
            .find(|(method, _, _)| {
                *method == KagemushaCoreCoordinatorMethodV1::AcceptAuthenticatedReply
            })
            .unwrap();
        let frame = kagemusha_core_coordinator_encode_request_v1(&fields).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_request_v1(method, &frame),
            Ok(())
        );
        assert_eq!(
            kagemusha_core_coordinator_decode_request_v1(&frame).unwrap()[4],
            fields[4]
        );
        // These are signature-shape fixtures only. The qualified backend must verify the
        // actual signed transcript under its admitted session key before accepting the reply.
        let mut retired = fields.clone();
        retired.remove(4);
        let retired = kagemusha_core_coordinator_encode_request_v1(&retired).unwrap();
        assert_eq!(
            kagemusha_core_coordinator_validate_method_request_v1(method, &retired),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
        );
        for invalid_signature in [
            vec![],
            vec![1; 63],
            vec![1; 65],
            vec![0; 64],
            vec![0xff; 64],
        ] {
            let mut invalid = fields.clone();
            invalid[4] = invalid_signature;
            let invalid = kagemusha_core_coordinator_encode_request_v1(&invalid).unwrap();
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(method, &invalid),
                Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
            );
        }
    }

    #[test]
    fn signed_app_discriminants_and_sender_variants_are_closed() {
        for operation in 1..=22_u32 {
            let fields = vec![u32_field(operation), digest(0x51), b"binding".to_vec()];
            let frame = kagemusha_core_coordinator_encode_request_v1(&fields).expect("frame");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(
                    KagemushaCoreCoordinatorMethodV1::ReserveOperationId,
                    &frame,
                ),
                if is_observation_operation_v1(operation) {
                    Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
                } else {
                    Ok(())
                }
            );
        }
        for invalid in [vec![1], u32_field(0), u32_field(23), vec![1, 0, 1, 0]] {
            let fields = vec![invalid, digest(0x51), b"binding".to_vec()];
            let frame = kagemusha_core_coordinator_encode_request_v1(&fields).expect("frame");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(
                    KagemushaCoreCoordinatorMethodV1::ReserveOperationId,
                    &frame,
                ),
                Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
            );
        }

        let mut send_as_redeem = send_begin_request_fields(digest(0x71));
        send_as_redeem[1] = u32_field(KAGEMUSHA_CORE_COORDINATOR_REDEEM_SPLIT_V1);
        let mut redeem_as_send = redeem_begin_request_fields(digest(0x72));
        redeem_as_send[1] = u32_field(KAGEMUSHA_CORE_COORDINATOR_SEND_SPLIT_V1);
        let mut zero_redeem = redeem_begin_request_fields(digest(0x73));
        zero_redeem[2] = vec![0; 16];
        for invalid in [send_as_redeem, redeem_as_send, zero_redeem] {
            let frame = kagemusha_core_coordinator_encode_request_v1(&invalid).expect("frame");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(
                    KagemushaCoreCoordinatorMethodV1::BeginSenderTransition,
                    &frame,
                ),
                Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
            );
        }

        let mut payment_with_redemption_receipt = send_release_request_fields();
        payment_with_redemption_receipt[4] =
            terminal_receipt_field(KAGEMUSHA_CORE_COORDINATOR_REDEEM_SPLIT_V1);
        let mut redemption_with_payment_receipt = redeem_release_request_fields();
        redemption_with_payment_receipt[5] =
            terminal_receipt_field(KAGEMUSHA_CORE_COORDINATOR_SEND_SPLIT_V1);
        for invalid in [
            payment_with_redemption_receipt,
            redemption_with_payment_receipt,
        ] {
            let frame = kagemusha_core_coordinator_encode_request_v1(&invalid).expect("frame");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(
                    KagemushaCoreCoordinatorMethodV1::ReleaseOutbox,
                    &frame,
                ),
                Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
            );
        }
    }

    #[test]
    fn signed_android_and_ios_responses_have_one_exact_method_matrix() {
        for (method, label, request_fields) in mobile_request_cases() {
            let request = kagemusha_core_coordinator_encode_request_v1(&request_fields)
                .expect("request frame");
            let response_fields = mobile_response_fields(method, &request_fields);
            let response = kagemusha_core_coordinator_encode_response_v1(&response_fields)
                .expect("response frame");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_response_v1(method, &request, &response,),
                Ok(()),
                "valid signed-app response for {method:?}/{label}"
            );

            if !response_fields.is_empty() {
                let missing = kagemusha_core_coordinator_encode_response_v1(
                    &response_fields[..response_fields.len() - 1],
                )
                .expect("missing response field");
                assert_eq!(
                    kagemusha_core_coordinator_validate_method_response_v1(
                        method, &request, &missing,
                    ),
                    Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
                    "missing response field for {method:?}/{label}"
                );
            }

            let mut trailing_fields = response_fields;
            trailing_fields.push(b"trailing".to_vec());
            let trailing = kagemusha_core_coordinator_encode_response_v1(&trailing_fields)
                .expect("trailing response field");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_response_v1(method, &request, &trailing,),
                Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
                "trailing response field for {method:?}/{label}"
            );
        }
    }

    #[test]
    fn response_validation_rejects_substitution_and_partial_release() {
        let begin_fields = send_begin_request_fields(digest(0x81));
        let begin =
            kagemusha_core_coordinator_encode_request_v1(&begin_fields).expect("begin request");
        let substituted_begin = kagemusha_core_coordinator_encode_response_v1(&[
            digest(0x82),
            b"canonical-preparation".to_vec(),
        ])
        .expect("substituted begin response");
        assert_eq!(
            kagemusha_core_coordinator_validate_method_response_v1(
                KagemushaCoreCoordinatorMethodV1::BeginSenderTransition,
                &begin,
                &substituted_begin,
            ),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
        );

        let recovery_fields = mobile_request_cases()
            .into_iter()
            .find(|(method, _, _)| *method == KagemushaCoreCoordinatorMethodV1::RecoverSender)
            .expect("recovery case")
            .2;
        let recovery = kagemusha_core_coordinator_encode_request_v1(&recovery_fields)
            .expect("recovery request");
        let substituted_recovery = kagemusha_core_coordinator_encode_response_v1(&[
            digest(0x83),
            digest(0x84),
            b"canonical-preparation".to_vec(),
        ])
        .expect("substituted recovery response");
        assert_eq!(
            kagemusha_core_coordinator_validate_method_response_v1(
                KagemushaCoreCoordinatorMethodV1::RecoverSender,
                &recovery,
                &substituted_recovery,
            ),
            Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
        );

        let release_fields = send_release_request_fields();
        let release =
            kagemusha_core_coordinator_encode_request_v1(&release_fields).expect("release request");
        for invalid_fields in [
            vec![
                vec![0; 32],
                b"canonical-preparation".to_vec(),
                digest(0x85),
                b"terminal-envelope".to_vec(),
                b"authorization".to_vec(),
            ],
            vec![
                digest(0x86),
                b"canonical-preparation".to_vec(),
                vec![0; 32],
                b"terminal-envelope".to_vec(),
                b"authorization".to_vec(),
            ],
            vec![
                digest(0x86),
                b"canonical-preparation".to_vec(),
                digest(0x87),
                Vec::new(),
                b"authorization".to_vec(),
            ],
            vec![
                digest(0x86),
                b"canonical-preparation".to_vec(),
                digest(0x87),
                b"terminal-envelope".to_vec(),
                Vec::new(),
            ],
        ] {
            let response = kagemusha_core_coordinator_encode_response_v1(&invalid_fields)
                .expect("invalid release response");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_response_v1(
                    KagemushaCoreCoordinatorMethodV1::ReleaseOutbox,
                    &release,
                    &response,
                ),
                Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
            );
        }
    }

    #[test]
    fn release_response_rejects_another_installed_envelope_for_both_sender_kinds() {
        let method = KagemushaCoreCoordinatorMethodV1::ReleaseOutbox;
        for request_fields in [
            send_release_request_fields(),
            redeem_release_request_fields(),
        ] {
            let request = kagemusha_core_coordinator_encode_request_v1(&request_fields)
                .expect("release request");
            let mut response_fields = mobile_response_fields(method, &request_fields);
            let response = kagemusha_core_coordinator_encode_response_v1(&response_fields)
                .expect("matching release response");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_response_v1(method, &request, &response),
                Ok(())
            );
            response_fields[3] = b"another-installed-envelope".to_vec();
            let substituted = kagemusha_core_coordinator_encode_response_v1(&response_fields)
                .expect("well-shaped substituted release response");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_response_v1(
                    method,
                    &request,
                    &substituted,
                ),
                Err(KagemushaCoreCoordinatorFrameErrorV1::Field)
            );
        }
    }

    #[test]
    fn storage_path_validation_requires_one_lexical_absolute_name() {
        assert_eq!(
            kagemusha_core_coordinator_validate_storage_path_v1(b"/durable/kagemusha.db"),
            Ok("/durable/kagemusha.db")
        );
        assert_eq!(
            kagemusha_core_coordinator_validate_storage_path_v1("/durable/🔒".as_bytes()),
            Ok("/durable/🔒")
        );
        for invalid in [
            &b""[..],
            &b" \t"[..],
            &b"bad\0path"[..],
            &[0xff][..],
            &b"relative/store"[..],
            &b"/"[..],
            &b"/durable/"[..],
            &b"/durable//store"[..],
            &b"/durable/./store"[..],
            &b"/durable/../store"[..],
            &b"/durable/sto\\re"[..],
            &b"/durable/sto\nre"[..],
            &b"/durable/sto\x7fre"[..],
        ] {
            assert!(kagemusha_core_coordinator_validate_storage_path_v1(invalid).is_err());
        }
        assert!(
            kagemusha_core_coordinator_validate_storage_path_v1(&vec![
                b'a';
                KAGEMUSHA_CORE_COORDINATOR_MAX_STORAGE_PATH_BYTES_V1
                    + 1
            ])
            .is_err()
        );
    }

    #[test]
    fn c_boundary_exports_exact_contract() {
        let mut contract = [0_u32; KAGEMUSHA_CORE_COORDINATOR_CONTRACT_WORDS_V1.len()];
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_contract_v1(
                    contract.as_mut_ptr(),
                    contract.len() - 1,
                )
            },
            crate::ERR_BUFFER_TOO_SMALL
        );
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_contract_v1(
                    contract.as_mut_ptr(),
                    contract.len(),
                )
            },
            contract.len() as libc::c_int
        );
        assert_eq!(contract, KAGEMUSHA_CORE_COORDINATOR_CONTRACT_WORDS_V1);
    }

    #[test]
    fn c_boundary_rejects_malformed_invocations_before_unavailable() {
        let request = kagemusha_core_coordinator_encode_request_v1(&[]).expect("request");
        let mut output_ptr = core::ptr::null_mut();
        let mut output_len = 0;
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_invoke_v1(
                    0,
                    1,
                    request.as_ptr(),
                    request.len(),
                    &mut output_ptr,
                    &mut output_len,
                )
            },
            crate::ERR_KAGEMUSHA_V1
        );
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_invoke_v1(
                    1,
                    12,
                    request.as_ptr(),
                    request.len(),
                    &mut output_ptr,
                    &mut output_len,
                )
            },
            crate::ERR_KAGEMUSHA_V1
        );
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_invoke_v1(
                    1,
                    1,
                    b"not-a-frame".as_ptr(),
                    b"not-a-frame".len(),
                    &mut output_ptr,
                    &mut output_len,
                )
            },
            crate::ERR_KAGEMUSHA_V1
        );
    }

    #[test]
    fn install_once_backend_is_bounded_and_cannot_be_replaced() {
        assert!(installed_kagemusha_core_coordinator_backend_v1().is_none());
        let storage_path = b"/durable/kagemusha.db";
        let mut handle = u64::MAX;
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_open_v1(
                    storage_path.as_ptr(),
                    storage_path.len(),
                    &mut handle,
                )
            },
            crate::ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1
        );
        assert_eq!(handle, 0);

        // BeginObservation is a canonical device-read command, not the paired State-proof
        // observer. Even a well-formed device read cannot acquire a native session or verifier
        // from this stock bridge: a release-authenticated Rust owner must be installed first.
        let observation_method = KagemushaCoreCoordinatorMethodV1::BeginObservation;
        let observation_request =
            kagemusha_core_coordinator_encode_request_v1(&observation_request_fields(21))
                .expect("canonical device observation request");
        assert_eq!(
            archive_boundary::validate_request(observation_method, &observation_request),
            Ok(())
        );
        let mut observation_output = core::ptr::null_mut();
        let mut observation_output_len = usize::MAX;
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_invoke_v1(
                    7,
                    observation_method.code(),
                    observation_request.as_ptr(),
                    observation_request.len(),
                    &mut observation_output,
                    &mut observation_output_len,
                )
            },
            crate::ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1
        );
        assert!(observation_output.is_null());
        assert_eq!(observation_output_len, 0);
        let proof_export_request = kagemusha_core_coordinator_encode_request_v1(&[digest(0x66)])
            .expect("original operation selector");
        assert_eq!(
            archive_boundary::validate_request(
                KagemushaCoreCoordinatorMethodV1::ExportOutgoingStateProof,
                &proof_export_request,
            ),
            Ok(())
        );
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_invoke_v1(
                    7,
                    KagemushaCoreCoordinatorMethodV1::ExportOutgoingStateProof.code(),
                    proof_export_request.as_ptr(),
                    proof_export_request.len(),
                    &mut observation_output,
                    &mut observation_output_len,
                )
            },
            crate::ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1
        );
        assert!(observation_output.is_null());
        assert_eq!(observation_output_len, 0);
        let mut forged_proof_slot = observation_request_fields(21);
        forged_proof_slot[1] = b"paired-state-proof".to_vec();
        let forged_proof_request = kagemusha_core_coordinator_encode_request_v1(&forged_proof_slot)
            .expect("bounded forged request");
        assert!(
            archive_boundary::validate_request(observation_method, &forged_proof_request).is_err()
        );

        // Transport-shaped opaque selectors must fail before either dispatch or unavailable.
        let mut preparation_fields = vec![b"opaque-preparation".to_vec(), b"reply".to_vec()];
        let mut output_ptr = core::ptr::null_mut();
        let mut output_len = usize::MAX;
        for expected in [
            crate::ERR_KAGEMUSHA_V1,
            crate::ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1,
        ] {
            let request =
                kagemusha_core_coordinator_encode_request_v1(&preparation_fields).unwrap();
            assert_eq!(
                unsafe {
                    crate::connect_norito_kagemusha_core_coordinator_invoke_v1(
                        7,
                        KagemushaCoreCoordinatorMethodV1::ProvePreparedSenderTransition.code(),
                        request.as_ptr(),
                        request.len(),
                        &mut output_ptr,
                        &mut output_len,
                    )
                },
                expected
            );
            assert!(output_ptr.is_null());
            assert_eq!(output_len, 0);
            let (_, canonical_release) = archive_boundary::tests::release_fields();
            preparation_fields[0] = canonical_release[1].clone();
        }

        let backend = Arc::new(TestBackend {
            invokes: AtomicUsize::new(0),
            closes: AtomicUsize::new(0),
            caller_path_pointer: AtomicUsize::new(0),
            caller_request_pointer: AtomicUsize::new(0),
            response: Mutex::new(TestResponse::ReserveValid),
        });
        install_kagemusha_core_coordinator_backend_v1(backend.clone()).expect("first install");
        assert_eq!(
            install_kagemusha_core_coordinator_backend_v1(Arc::new(TestBackend {
                invokes: AtomicUsize::new(0),
                closes: AtomicUsize::new(0),
                caller_path_pointer: AtomicUsize::new(0),
                caller_request_pointer: AtomicUsize::new(0),
                response: Mutex::new(TestResponse::ReserveValid),
            })),
            Err(KagemushaCoreCoordinatorInstallErrorV1::AlreadyInstalled)
        );

        backend
            .caller_path_pointer
            .store(storage_path.as_ptr() as usize, Ordering::SeqCst);
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_open_v1(
                    storage_path.as_ptr(),
                    storage_path.len(),
                    &mut handle,
                )
            },
            0
        );
        assert_eq!(handle, 7);

        let request = kagemusha_core_coordinator_encode_request_v1(&[
            u32_field(5),
            digest(0x51),
            canonical_sender_reservation_binding(),
        ])
        .expect("canonical request");
        let mut output_ptr = core::ptr::null_mut();
        let mut output_len = 0_usize;
        backend
            .caller_request_pointer
            .store(request.as_ptr() as usize, Ordering::SeqCst);
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_invoke_v1(
                    handle,
                    KagemushaCoreCoordinatorMethodV1::ReserveOperationId.code(),
                    request.as_ptr(),
                    request.len(),
                    &mut output_ptr,
                    &mut output_len,
                )
            },
            0
        );
        let output = unsafe { core::slice::from_raw_parts(output_ptr, output_len) }.to_vec();
        crate::connect_norito_free(output_ptr);
        assert_eq!(
            kagemusha_core_coordinator_decode_response_v1(&output),
            Ok(vec![digest(0x51)])
        );
        assert_eq!(backend.invokes.load(Ordering::SeqCst), 1);

        let mut malformed_request = request.clone();
        malformed_request[0] ^= 1;
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_invoke_v1(
                    handle,
                    KagemushaCoreCoordinatorMethodV1::ReserveOperationId.code(),
                    malformed_request.as_ptr(),
                    malformed_request.len(),
                    &mut output_ptr,
                    &mut output_len,
                )
            },
            crate::ERR_KAGEMUSHA_V1
        );
        assert_eq!(backend.invokes.load(Ordering::SeqCst), 1);

        let (release_fields, release_response) = archive_boundary::tests::release_fields();
        let release = kagemusha_core_coordinator_encode_request_v1(&release_fields)
            .expect("canonical release request");
        *backend.response.lock().expect("response mode") = TestResponse::ReleaseValid;
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_invoke_v1(
                    handle,
                    KagemushaCoreCoordinatorMethodV1::ReleaseOutbox.code(),
                    release.as_ptr(),
                    release.len(),
                    &mut output_ptr,
                    &mut output_len,
                )
            },
            0
        );
        let output = unsafe { core::slice::from_raw_parts(output_ptr, output_len) }.to_vec();
        crate::connect_norito_free(output_ptr);
        assert_eq!(
            kagemusha_core_coordinator_decode_response_v1(&output),
            Ok(release_response)
        );

        *backend.response.lock().expect("response mode") = TestResponse::ReleaseSubstituted;
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_invoke_v1(
                    handle,
                    KagemushaCoreCoordinatorMethodV1::ReleaseOutbox.code(),
                    release.as_ptr(),
                    release.len(),
                    &mut output_ptr,
                    &mut output_len,
                )
            },
            crate::ERR_KAGEMUSHA_V1
        );
        assert!(output_ptr.is_null());
        assert_eq!(output_len, 0);

        let invokes_before_revocation = backend.invokes.load(Ordering::SeqCst);
        let acknowledgment_request = kagemusha_core_coordinator_encode_request_v1(&[
            digest(0x11),
            b"app-attest-key".to_vec(),
            app_attest_selection_for_tests(),
            app_attest_assertion_for_tests(11),
            4_u32.to_le_bytes().to_vec(),
            digest(0x33),
            digest(0x44),
        ])
        .expect("canonical acknowledgment request");
        let mut acknowledgment_output = core::ptr::null_mut();
        let mut acknowledgment_output_len = usize::MAX;
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_invoke_v1(
                    handle,
                    KagemushaCoreCoordinatorMethodV1::AcknowledgeCommittedAppAttest.code(),
                    acknowledgment_request.as_ptr(),
                    acknowledgment_request.len(),
                    &mut acknowledgment_output,
                    &mut acknowledgment_output_len,
                )
            },
            crate::ERR_KAGEMUSHA_V1
        );
        assert!(acknowledgment_output.is_null());
        assert_eq!(acknowledgment_output_len, 0);
        assert_eq!(
            crate::connect_norito_kagemusha_core_coordinator_close_v1(handle),
            crate::ERR_KAGEMUSHA_V1
        );
        assert_eq!(backend.closes.load(Ordering::SeqCst), 1);
        assert_eq!(
            crate::connect_norito_kagemusha_core_coordinator_close_v1(handle),
            crate::ERR_KAGEMUSHA_V1
        );
        let mut second_handle = u64::MAX;
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_open_v1(
                    storage_path.as_ptr(),
                    storage_path.len(),
                    &mut second_handle,
                )
            },
            crate::ERR_KAGEMUSHA_V1
        );
        assert_eq!(second_handle, 0);
        assert_eq!(
            unsafe {
                crate::connect_norito_kagemusha_core_coordinator_invoke_v1(
                    handle,
                    KagemushaCoreCoordinatorMethodV1::ReserveOperationId.code(),
                    request.as_ptr(),
                    request.len(),
                    &mut output_ptr,
                    &mut output_len,
                )
            },
            crate::ERR_KAGEMUSHA_V1
        );
        assert!(output_ptr.is_null());
        assert_eq!(output_len, 0);
        assert_eq!(
            backend.invokes.load(Ordering::SeqCst),
            invokes_before_revocation
        );
        assert_eq!(backend.closes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn header_and_jni_names_pin_the_kagemusha_only_boundary() {
        let header = include_str!("../include/connect_norito_bridge.h");
        let compact: String = header.split_whitespace().collect();
        let header_methods = compact
            .split_once("typedefenumConnectNoritoKagemushaCoreCoordinatorMethodV1{")
            .expect("C coordinator method enum")
            .1
            .split_once("}ConnectNoritoKagemushaCoreCoordinatorMethodV1;")
            .expect("C coordinator method enum end")
            .0
            .split(',')
            .filter(|entry| !entry.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let native_methods = KagemushaCoreCoordinatorMethodV1::ALL
            .map(|method| {
                let mut name = String::from("CONNECT_NORITO_KAGEMUSHA_CORE_COORDINATOR_");
                for (index, character) in format!("{method:?}").chars().enumerate() {
                    if index != 0 && character.is_ascii_uppercase() {
                        name.push('_');
                    }
                    name.push(character.to_ascii_uppercase());
                }
                format!("{name}_V1={}", method.code())
            })
            .to_vec();
        assert_eq!(
            header_methods, native_methods,
            "C coordinator method inventory"
        );
        let version = KAGEMUSHA_CORE_COORDINATOR_FRAME_VERSION_V1;
        assert!(header.contains(&format!(
            "#define CONNECT_NORITO_KAGEMUSHA_CORE_COORDINATOR_FRAME_VERSION_V1 UINT16_C({version})"
        )));
        let swift = include_str!(
            "../../../IrohaSwift/Sources/IrohaSwift/KagemushaCoreCoordinatorFrameV1.swift"
        );
        assert!(swift.contains(&format!(
            "public static let schemaVersion: UInt16 = {version}"
        )));
        assert!(header.contains(
            "CONNECT_NORITO_KAGEMUSHA_CORE_COORDINATOR_EXPORT_OUTGOING_STATE_PROOF_V1 = 14"
        ));
        assert!(swift.contains("case exportOutgoingStateProof"));
        let kotlin = include_str!(
            "../../../kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/KagemushaCoreCoordinatorFrameV1.kt"
        );
        assert!(kotlin.contains("EXPORT_OUTGOING_STATE_PROOF(14)"));
        for symbol in [
            "connect_norito_kagemusha_core_coordinator_contract_v1(",
            "connect_norito_kagemusha_core_coordinator_open_v1(",
            "connect_norito_kagemusha_core_coordinator_invoke_v1(",
            "connect_norito_kagemusha_core_coordinator_close_v1(",
        ] {
            assert!(header.contains(symbol));
        }
        let source = crate::bridge_source();
        assert!(source.contains("backend.invoke_checked_with_output("));
        assert!(
            include_str!("kagemusha_core_coordinator_v1/exclusive_backend.rs")
                .contains("KagemushaCoreCoordinatorMethodV1::ExportOutgoingStateProof =>")
        );
        for symbol in [
            "Java_org_hyperledger_iroha_sdk_offline_KagemushaCoreCoordinatorJniV1_nativeContractV1",
            "Java_org_hyperledger_iroha_sdk_offline_KagemushaCoreCoordinatorJniV1_nativeOpenV1",
            "Java_org_hyperledger_iroha_sdk_offline_KagemushaCoreCoordinatorJniV1_nativeInvokeV1",
            "Java_org_hyperledger_iroha_sdk_offline_KagemushaCoreCoordinatorJniV1_nativeCloseV1",
        ] {
            let declaration = format!("pub extern \"system\" fn {symbol}(");
            assert_eq!(source.matches(&declaration).count(), 1);
        }
        assert!(!source.contains("Java_pg_bpng_digitalkina_"));
        let retired_identity: String = "1VinJeroCevitaNenilffO".chars().rev().collect();
        assert!(!source.contains(&retired_identity));
    }
}
