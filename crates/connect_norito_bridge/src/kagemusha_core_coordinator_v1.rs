//! Strict native frame contract for a qualified KAGEMUSHA Core coordinator.
//!
//! This module defines framing and inventory only. The generic bridge does not
//! synthesize a coordinator, monetary result, storage handle, or hardware
//! authority. Its exported open/invoke functions therefore validate their
//! inputs and fail closed until a qualified platform build supplies the
//! authenticated durable coordinator.

mod archives;
pub use archives::{
    KAGEMUSHA_CORE_COORDINATOR_ARCHIVE_MAX_BYTES_V1, KagemushaCoreCoordinatorArchiveErrorV1,
    KagemushaCoreSenderCandidateArchiveV1, KagemushaCoreSenderPreparationArchiveV1,
    KagemushaCoreSenderRecoveryArchiveV1,
};
pub use crate::kagemusha_device_bridge_v1::sender_payload::{
    SenderPreparationSelectorV1 as KagemushaCoreSenderPreparationSelectorV1,
    SenderWalletContextV1 as KagemushaCoreSenderWalletContextV1,
};

use iroha_data_model::kagemusha::{
    KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1, KagemushaArtifactRoleV1,
    KagemushaQualifiedHelperCircuitV1, KagemushaQualifiedRelationV1,
};
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
pub const KAGEMUSHA_CORE_COORDINATOR_MAX_FIELD_BYTES_V1: usize = 64 * 1024;
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

/// Exact native coordinator contract returned as ten `u32` words.
pub const KAGEMUSHA_CORE_COORDINATOR_CONTRACT_WORDS_V1: [u32; 10] = [
    KAGEMUSHA_CORE_COORDINATOR_FRAME_VERSION_V1 as u32,
    CONNECT_NORITO_BRIDGE_ABI_VERSION,
    CONNECT_NORITO_KAGEMUSHA_IPM1_MESSAGE_KIND_TAGS_V1.len() as u32,
    KAGEMUSHA_CORE_COORDINATOR_WIRE_PAYLOAD_COUNT_V1,
    KagemushaArtifactRoleV1::ALL.len() as u32,
    KagemushaQualifiedRelationV1::ALL.len() as u32,
    KagemushaQualifiedHelperCircuitV1::ALL.len() as u32,
    KagemushaDeviceLifecycleOperationV1::ALL.len() as u32,
    KAGEMUSHA_NATIVE_HARDWARE_CAPABILITY_BITS_V1.len() as u32,
    KAGEMUSHA_HARDWARE_REQUIRED_CAPABILITIES_V1 as u32,
];

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
}

impl KagemushaCoreCoordinatorMethodV1 {
    /// All coordinator methods in canonical code order.
    pub const ALL: [Self; 10] = [
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
}

static KAGEMUSHA_CORE_COORDINATOR_BACKEND_V1: OnceLock<Arc<dyn KagemushaCoreCoordinatorBackendV1>> =
    OnceLock::new();

/// Install the qualified coordinator backend exactly once for this process.
///
/// Stock builds never call this function. There is intentionally no uninstall,
/// overwrite, software implementation, or C/JNI installer.
pub fn install_kagemusha_core_coordinator_backend_v1(
    backend: Arc<dyn KagemushaCoreCoordinatorBackendV1>,
) -> Result<(), KagemushaCoreCoordinatorInstallErrorV1> {
    KAGEMUSHA_CORE_COORDINATOR_BACKEND_V1
        .set(backend)
        .map_err(|_| KagemushaCoreCoordinatorInstallErrorV1::AlreadyInstalled)
}

pub(crate) fn installed_kagemusha_core_coordinator_backend_v1()
-> Option<&'static dyn KagemushaCoreCoordinatorBackendV1> {
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

/// Validate method-specific request bindings before dispatching to a platform backend.
///
/// BeginSenderTransition field zero is always the caller-persisted nonzero operation ID.
/// RecoverSender fields zero and one are respectively the closed selector and selected nonzero ID.
/// All integer discriminants use canonical little-endian `u32` fields, matching both signed apps.
pub fn kagemusha_core_coordinator_validate_method_request_v1(
    method: KagemushaCoreCoordinatorMethodV1,
    frame: &[u8],
) -> Result<(), KagemushaCoreCoordinatorFrameErrorV1> {
    let fields = kagemusha_core_coordinator_decode_request_v1(frame)?;
    match method {
        KagemushaCoreCoordinatorMethodV1::ReserveOperationId => {
            require_field_count(&fields, 3)?;
            require_device_operation_field(fields.first())?;
            require_nonzero_digest_field(fields.get(1))?;
            require_nonempty_field(fields.get(2))
        }
        KagemushaCoreCoordinatorMethodV1::AcceptQualification => {
            require_field_count(&fields, 6)?;
            require_qualification_fields(&fields, 0)?;
            require_nonzero_digest_field(fields.get(5))
        }
        KagemushaCoreCoordinatorMethodV1::AcceptAuthenticatedReply => {
            require_field_count(&fields, 9)?;
            require_device_operation_field(fields.first())?;
            require_nonzero_digest_field(fields.get(1))?;
            require_nonempty_field(fields.get(2))?;
            require_nonempty_field(fields.get(3))?;
            require_qualification_fields(&fields, 4)
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
        KagemushaCoreCoordinatorMethodV1::ProvePreparedSenderTransition
        | KagemushaCoreCoordinatorMethodV1::BuildTerminalEnvelope
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
    }
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

/// Validate the exact response shape for one already validated method request.
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
        KagemushaCoreCoordinatorMethodV1::ReserveOperationId => {
            require_field_count(&response, 1)?;
            require_nonzero_digest_field(response.first())?;
            require_equal_fields(response.first(), request.get(1))
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

/// Validate a durable-store path without opening or creating any storage.
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
    if path.trim().is_empty() {
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
        InvalidSchema,
        Malformed,
        Oversized,
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
                    b"canonical-candidate".to_vec(),
                    b"authenticated-reply".to_vec(),
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
        ]
    }

    fn mobile_response_fields(
        method: KagemushaCoreCoordinatorMethodV1,
        request: &[Vec<u8>],
    ) -> Vec<Vec<u8>> {
        match method {
            KagemushaCoreCoordinatorMethodV1::ReserveOperationId => vec![request[1].clone()],
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
        }
    }

    #[test]
    fn shared_sdk_frames_match_every_native_method_and_recovery_selector() {
        let fixture = include_str!(
            "../../../fixtures/offline/kagemusha_core_coordinator_frame_v1.tsv"
        );
        let mut fixtures = std::collections::BTreeMap::new();
        for line in fixture.lines().filter(|line| !line.starts_with('#') && !line.is_empty()) {
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
            assert!(fixtures.insert(columns[0], (method, request, response)).is_none());
        }
        assert_eq!(fixtures.len(), 14);
        for (method, name, request_fields) in mobile_request_cases() {
            let (actual_method, request, response) = fixtures.get(name).expect("shared method case");
            assert_eq!(*actual_method, method);
            assert_eq!(
                *request,
                kagemusha_core_coordinator_encode_request_v1(&request_fields).expect("native request"),
                "{name}",
            );
            assert_eq!(
                *response,
                kagemusha_core_coordinator_encode_response_v1(&mobile_response_fields(method, &request_fields))
                    .expect("native response"),
                "{name}",
            );
        }
        let (_, missing_request, missing_response) = fixtures.get("recover-missing").unwrap();
        let missing_fields = kagemusha_core_coordinator_decode_request_v1(missing_request).unwrap();
        assert_eq!(missing_fields[0], [KAGEMUSHA_CORE_COORDINATOR_RECOVER_BY_OPERATION_ID_V1]);
        assert!(kagemusha_core_coordinator_decode_response_v1(missing_response).unwrap().is_empty());
        let (_, terminal_request, _) = fixtures.get("recover-terminal").unwrap();
        let terminal_fields = kagemusha_core_coordinator_decode_request_v1(terminal_request).unwrap();
        assert_eq!(terminal_fields[0], [KAGEMUSHA_CORE_COORDINATOR_RECOVER_BY_TERMINAL_ID_V1]);
    }

    struct TestBackend {
        invokes: AtomicUsize,
        response: Mutex<TestResponse>,
    }

    impl KagemushaCoreCoordinatorBackendV1 for TestBackend {
        fn open(&self, storage_path: &str) -> Result<u64, KagemushaCoreCoordinatorBackendErrorV1> {
            assert_eq!(storage_path, "/durable/kagemusha.db");
            Ok(7)
        }

        fn invoke(
            &self,
            handle: u64,
            method: KagemushaCoreCoordinatorMethodV1,
            request_frame: &[u8],
        ) -> Result<Vec<u8>, KagemushaCoreCoordinatorBackendErrorV1> {
            assert_eq!(handle, 7);
            self.invokes.fetch_add(1, Ordering::SeqCst);
            match *self.response.lock().expect("response mode") {
                TestResponse::ReserveValid => {
                    assert_eq!(method, KagemushaCoreCoordinatorMethodV1::ReserveOperationId);
                    assert_eq!(
                        kagemusha_core_coordinator_decode_request_v1(request_frame),
                        Ok(vec![u32_field(1), digest(0x51), b"input".to_vec()])
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
                    let mut fields = mobile_response_fields(
                        method,
                        &kagemusha_core_coordinator_decode_request_v1(request_frame)
                            .expect("request"),
                    );
                    if matches!(mode, TestResponse::ReleaseSubstituted) {
                        fields[3] = b"another-installed-envelope".to_vec();
                    }
                    kagemusha_core_coordinator_encode_response_v1(&fields)
                        .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)
                }
                TestResponse::InvalidSchema => {
                    let invalid = match method {
                        KagemushaCoreCoordinatorMethodV1::ReserveOperationId => {
                            vec![b"not-a-digest".to_vec()]
                        }
                        KagemushaCoreCoordinatorMethodV1::ReleaseOutbox => vec![
                            digest(0x63),
                            b"canonical-preparation".to_vec(),
                            digest(0x64),
                            b"terminal-envelope".to_vec(),
                        ],
                        _ => panic!("unexpected test method"),
                    };
                    kagemusha_core_coordinator_encode_response_v1(&invalid)
                        .map_err(|_| KagemushaCoreCoordinatorBackendErrorV1::Rejected)
                }
                TestResponse::Malformed => Ok(b"not-a-frame".to_vec()),
                TestResponse::Oversized => Ok(vec![
                    0;
                    KAGEMUSHA_CORE_COORDINATOR_MAX_RESPONSE_BYTES_V1
                        + 1
                ]),
            }
        }
    }

    #[test]
    fn coordinator_contract_and_methods_are_exact() {
        assert_eq!(
            KAGEMUSHA_CORE_COORDINATOR_CONTRACT_WORDS_V1,
            [2, 23, 3, 6, 50, 8, 6, 22, 16, 0xffff]
        );
        assert_eq!(
            KagemushaCoreCoordinatorMethodV1::ALL.map(KagemushaCoreCoordinatorMethodV1::code),
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
        );
        for method in KagemushaCoreCoordinatorMethodV1::ALL {
            assert_eq!(
                KagemushaCoreCoordinatorMethodV1::from_code(method.code()),
                Some(method)
            );
        }
        for unknown in [0, 11, u8::MAX] {
            assert_eq!(KagemushaCoreCoordinatorMethodV1::from_code(unknown), None);
        }
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
    fn signed_android_and_ios_requests_have_one_exact_method_matrix() {
        let expected_counts = [3, 6, 9, 8, 9, 2, 2, 5, 8, 2, 10, 11];
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

            let missing = kagemusha_core_coordinator_encode_request_v1(
                &fields[..fields.len().saturating_sub(1)],
            )
            .expect("missing-field frame");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(method, &missing),
                Err(KagemushaCoreCoordinatorFrameErrorV1::Field),
                "missing field for {method:?}/{label}"
            );

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
    fn signed_app_discriminants_and_sender_variants_are_closed() {
        for operation in 1..=22_u32 {
            let fields = vec![u32_field(operation), digest(0x51), b"binding".to_vec()];
            let frame = kagemusha_core_coordinator_encode_request_v1(&fields).expect("frame");
            assert_eq!(
                kagemusha_core_coordinator_validate_method_request_v1(
                    KagemushaCoreCoordinatorMethodV1::ReserveOperationId,
                    &frame,
                ),
                Ok(())
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
    fn storage_path_validation_is_bounded_utf8_and_nul_free() {
        assert_eq!(
            kagemusha_core_coordinator_validate_storage_path_v1(b"/durable/kagemusha.db"),
            Ok("/durable/kagemusha.db")
        );
        for invalid in [&b""[..], &b" \t"[..], &b"bad\0path"[..], &[0xff][..]] {
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
                    11,
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

        let backend = Arc::new(TestBackend {
            invokes: AtomicUsize::new(0),
            response: Mutex::new(TestResponse::ReserveValid),
        });
        install_kagemusha_core_coordinator_backend_v1(backend.clone()).expect("first install");
        assert_eq!(
            install_kagemusha_core_coordinator_backend_v1(Arc::new(TestBackend {
                invokes: AtomicUsize::new(0),
                response: Mutex::new(TestResponse::ReserveValid),
            })),
            Err(KagemushaCoreCoordinatorInstallErrorV1::AlreadyInstalled)
        );

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
            u32_field(1),
            digest(0x51),
            b"input".to_vec(),
        ])
        .expect("canonical request");
        let mut output_ptr = core::ptr::null_mut();
        let mut output_len = 0_usize;
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

        *backend.response.lock().expect("response mode") = TestResponse::Malformed;
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

        *backend.response.lock().expect("response mode") = TestResponse::Oversized;
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

        *backend.response.lock().expect("response mode") = TestResponse::InvalidSchema;
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

        let release_fields = send_release_request_fields();
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
            Ok(mobile_response_fields(
                KagemushaCoreCoordinatorMethodV1::ReleaseOutbox,
                &release_fields,
            ))
        );

        for mode in [
            TestResponse::InvalidSchema,
            TestResponse::ReleaseSubstituted,
        ] {
            *backend.response.lock().expect("response mode") = mode;
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
        }
    }

    #[test]
    fn header_and_jni_names_pin_the_kagemusha_only_boundary() {
        let header = include_str!("../include/connect_norito_bridge.h");
        for symbol in [
            "connect_norito_kagemusha_core_coordinator_contract_v1(",
            "connect_norito_kagemusha_core_coordinator_open_v1(",
            "connect_norito_kagemusha_core_coordinator_invoke_v1(",
        ] {
            assert!(header.contains(symbol));
        }
        let source = crate::bridge_source();
        for symbol in [
            "Java_pg_bpng_digitalkina_KagemushaNativeCoreJniV1_nativeContractV1",
            "Java_pg_bpng_digitalkina_KagemushaNativeCoreJniV1_nativeOpenV1",
            "Java_pg_bpng_digitalkina_KagemushaNativeCoreJniV1_nativeInvokeV1",
        ] {
            assert!(source.contains(symbol));
        }
        let retired_identity: String = "1VinJeroCevitaNenilffO".chars().rev().collect();
        assert!(!source.contains(&retired_identity));
    }
}
