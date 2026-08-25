//! Core-owned typed ABI for the Offline Cash V1 secure-device lifecycle.
//!
//! The mobile SDKs frame commands for an optional platform service, but only
//! this module may translate Core's private journal/outbox types to that wire.
//! The transport is sealed: application code cannot manufacture a successful
//! response or deserialize a move-only Core capability. A backend becomes
//! usable only after its exact 96-byte capability frame and separate platform
//! evidence authenticate against the wallet's pinned identity.

use core::fmt;

use iroha_data_model::offline::{
    KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2, KagemushaDevicePublicKeyV2, KagemushaDeviceSignatureV2,
    OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1,
    OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1,
};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroizing;

use super::{
    AuthenticatedPaymentOutboxBackendV1, AuthenticatedPaymentOutboxErrorV1,
    AuthenticatedPaymentOutboxRecordV1, Digest, ExactNextHardwareGuardBackendV1,
    HardwareGuardErrorV1, HardwareIntentChallengeV1, HardwareIntentKindV1,
    HardwareTerminalOperationV1, PaymentOutboxKeyV1, digest_framed,
    guard::{
        HardwareActiveIntentOutcomeV1, HardwareGuardRequestV1, HardwareIntentCommitRequestV1,
        HardwareIntentRequestV1, HardwareReceiveSigningResultV1, HardwareReceiveTerminalQueryV1,
        HardwareTerminalOutcomeV1,
    },
    outbox::PaymentOutboxPublicationV1,
};

pub(crate) const DEVICE_LIFECYCLE_PROTOCOL_VERSION_V1: u16 = 1;
pub(crate) const DEVICE_LIFECYCLE_OPERATION_COUNT_V1: usize = 14;
pub(crate) const DEVICE_LIFECYCLE_CAPABILITY_FRAME_BYTES_V1: usize = 96;
pub(crate) const DEVICE_LIFECYCLE_REQUIRED_CAPABILITY_MASK_V1: u32 = 0x01ff;
pub(crate) const DEVICE_LIFECYCLE_COMMAND_HEADER_BYTES_V1: usize = 80;
pub(crate) const DEVICE_LIFECYCLE_RESPONSE_HEADER_BYTES_V1: usize = 116;
pub(crate) const DEVICE_LIFECYCLE_MAX_COMMAND_PAYLOAD_BYTES_V1: usize = 64 * 1024;
pub(crate) const DEVICE_LIFECYCLE_MAX_RESPONSE_PAYLOAD_BYTES_V1: usize = 64 * 1024;
pub(crate) const DEVICE_LIFECYCLE_MAX_AUTHENTICATOR_BYTES_V1: usize = 8 * 1024;
pub(crate) const DEVICE_LIFECYCLE_MAX_ATTESTATION_EVIDENCE_BYTES_V1: usize = 8 * 1024;

const CAPABILITY_MAGIC: &[u8; 8] = b"IOCFJCP1";
const COMMAND_MAGIC: &[u8; 8] = b"IOCFJCM1";
const RESPONSE_MAGIC: &[u8; 8] = b"IOCFJRS1";
const COMMAND_PAYLOAD_MAGIC: &[u8; 8] = b"IOCFJPC1";
const RESPONSE_PAYLOAD_MAGIC: &[u8; 8] = b"IOCFJPS1";
const TYPED_PAYLOAD_HEADER_BYTES: usize = 12;
const REQUEST_ID_DOMAIN: &[u8] = b"iroha:offline-cash:v1:device-lifecycle-request-id";
const RESPONSE_AUTHENTICATION_DOMAIN: &[u8] =
    b"iroha:offline-cash:v1:device-lifecycle-response-authentication";
const INTENT_CHALLENGE_DOMAIN: &[u8] = b"iroha:offline-cash:v1:hardware-intent";
const RESPONSE_AUTHENTICATOR_DIGEST_OFFSET: usize = 84;

/// Exact platform discriminator in the 96-byte capability frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum DeviceLifecyclePlatformV1 {
    Android = 1,
    Ios = 2,
}

/// Closed operation inventory shared by the two sealed Core backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum DeviceLifecycleOperationV1 {
    ReserveReceiveIntentAndSign = 1,
    RecoverReceiveIntentAndSignature = 2,
    BindReceiveRequestDigest = 3,
    PublishSendPayment = 4,
    RecoverActiveIntent = 5,
    CancelExpiredReceive = 6,
    CommitIntentExactNext = 7,
    RecoverTerminal = 8,
    RecoverReceiveTerminal = 9,
    SignReceiveAcknowledgement = 10,
    StagePayment = 11,
    RecoverStagedPaymentDigest = 12,
    PublishStagedPayment = 13,
    RecoverPublishedPayment = 14,
}

impl DeviceLifecycleOperationV1 {
    pub(crate) const ALL: [Self; DEVICE_LIFECYCLE_OPERATION_COUNT_V1] = [
        Self::ReserveReceiveIntentAndSign,
        Self::RecoverReceiveIntentAndSignature,
        Self::BindReceiveRequestDigest,
        Self::PublishSendPayment,
        Self::RecoverActiveIntent,
        Self::CancelExpiredReceive,
        Self::CommitIntentExactNext,
        Self::RecoverTerminal,
        Self::RecoverReceiveTerminal,
        Self::SignReceiveAcknowledgement,
        Self::StagePayment,
        Self::RecoverStagedPaymentDigest,
        Self::PublishStagedPayment,
        Self::RecoverPublishedPayment,
    ];

    fn from_u8(value: u8) -> Result<Self, DeviceLifecycleBridgeErrorV1> {
        Self::ALL
            .into_iter()
            .find(|operation| *operation as u8 == value)
            .ok_or(DeviceLifecycleBridgeErrorV1::MalformedFrame)
    }
}

/// Exact stable result taxonomy used by the platform bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum DeviceLifecycleStatusV1 {
    Success = 0,
    Unavailable = 1,
    StaleOrConcurrent = 2,
    IntentMismatch = 3,
    TrustedTimeRejected = 4,
    PolicyRejected = 5,
    Missing = 6,
    Conflict = 7,
    Corrupt = 8,
    MalformedRequest = 9,
}

impl DeviceLifecycleStatusV1 {
    pub(crate) const ALL: [Self; 10] = [
        Self::Success,
        Self::Unavailable,
        Self::StaleOrConcurrent,
        Self::IntentMismatch,
        Self::TrustedTimeRejected,
        Self::PolicyRejected,
        Self::Missing,
        Self::Conflict,
        Self::Corrupt,
        Self::MalformedRequest,
    ];

    fn from_u8(value: u8) -> Result<Self, DeviceLifecycleBridgeErrorV1> {
        Self::ALL
            .into_iter()
            .find(|status| *status as u8 == value)
            .ok_or(DeviceLifecycleBridgeErrorV1::MalformedFrame)
    }
}

/// Pinned identity that must match an authenticated platform capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeviceLifecycleExpectedIdentityV1 {
    platform: DeviceLifecyclePlatformV1,
    hardware_policy_id: Digest,
    attestation_digest: Digest,
}

impl DeviceLifecycleExpectedIdentityV1 {
    pub(crate) fn new(
        platform: DeviceLifecyclePlatformV1,
        hardware_policy_id: Digest,
        attestation_digest: Digest,
    ) -> Result<Self, DeviceLifecycleBridgeErrorV1> {
        if !valid_digest(hardware_policy_id)
            || !valid_digest(attestation_digest)
            || hardware_policy_id == attestation_digest
        {
            return Err(DeviceLifecycleBridgeErrorV1::CapabilityMismatch);
        }
        Ok(Self {
            platform,
            hardware_policy_id,
            attestation_digest,
        })
    }
}

/// Structurally valid and platform-authenticated capability identity.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct AuthenticatedDeviceLifecycleCapabilitiesV1 {
    platform: DeviceLifecyclePlatformV1,
    hardware_policy_id: Digest,
    attestation_digest: Digest,
}

impl fmt::Debug for AuthenticatedDeviceLifecycleCapabilitiesV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedDeviceLifecycleCapabilitiesV1")
            .field("platform", &self.platform)
            .field("hardware_policy_id", &self.hardware_policy_id)
            .field("attestation_digest", &self.attestation_digest)
            .finish()
    }
}

/// Fail-closed bridge and authentication errors retained before trait mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeviceLifecycleBridgeErrorV1 {
    Unavailable,
    CapabilityMismatch,
    CapabilityAuthentication,
    InvalidTypedPayload,
    MalformedFrame,
    ResponseAuthentication,
    RemoteStatus(DeviceLifecycleStatusV1),
}

impl fmt::Display for DeviceLifecycleBridgeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "offline-cash device lifecycle is unavailable",
            Self::CapabilityMismatch => "offline-cash device capability mismatch",
            Self::CapabilityAuthentication => {
                "offline-cash device capability authentication failed"
            }
            Self::InvalidTypedPayload => "invalid typed offline-cash device payload",
            Self::MalformedFrame => "malformed offline-cash device frame",
            Self::ResponseAuthentication => "offline-cash device response authentication failed",
            Self::RemoteStatus(_) => "offline-cash device operation was rejected",
        })
    }
}

impl std::error::Error for DeviceLifecycleBridgeErrorV1 {}

mod transport_sealed {
    pub trait Sealed {}
}

/// Authenticated platform transport implemented only by reviewed Core modules.
///
/// `authenticate_capabilities` verifies the separate platform evidence, not
/// merely the self-asserted digests in the 96-byte frame. Likewise,
/// `authenticate_response` must validate the policy-specific authenticator over
/// the exact domain-separated digest supplied by Core before the adapter
/// decodes a success. That digest binds the authenticated capability identity,
/// complete command, response fields through the payload digest, and complete
/// response payload. It deliberately omits the authenticator digest and bytes,
/// avoiding a circular signature/MAC preimage.
pub(crate) trait AuthenticatedDeviceLifecycleTransportV1: transport_sealed::Sealed {
    fn capabilities_frame(&self) -> Result<Zeroizing<Vec<u8>>, DeviceLifecycleBridgeErrorV1>;

    fn authenticate_capabilities(
        &self,
        frame: &[u8; DEVICE_LIFECYCLE_CAPABILITY_FRAME_BYTES_V1],
        attestation_evidence: &[u8],
    ) -> Result<(), DeviceLifecycleBridgeErrorV1>;

    fn execute(&self, command: &[u8]) -> Result<Zeroizing<Vec<u8>>, DeviceLifecycleBridgeErrorV1>;

    fn authenticate_response(
        &self,
        capabilities: &AuthenticatedDeviceLifecycleCapabilitiesV1,
        response_authentication_digest: Digest,
        authenticator: &[u8],
    ) -> Result<(), DeviceLifecycleBridgeErrorV1>;
}

/// The sole adapter that implements both sealed lifecycle backends.
pub(crate) struct DeviceLifecycleBackendAdapterV1<T> {
    transport: T,
    capabilities: AuthenticatedDeviceLifecycleCapabilitiesV1,
}

impl<T> DeviceLifecycleBackendAdapterV1<T>
where
    T: AuthenticatedDeviceLifecycleTransportV1,
{
    /// Authenticate one exact backend identity before any lifecycle call.
    pub(crate) fn authenticate(
        transport: T,
        expected: DeviceLifecycleExpectedIdentityV1,
        attestation_evidence: &[u8],
    ) -> Result<Self, DeviceLifecycleBridgeErrorV1> {
        if attestation_evidence.is_empty()
            || attestation_evidence.len() > DEVICE_LIFECYCLE_MAX_ATTESTATION_EVIDENCE_BYTES_V1
            || sha256(attestation_evidence) != expected.attestation_digest
        {
            return Err(DeviceLifecycleBridgeErrorV1::CapabilityAuthentication);
        }
        let frame = transport.capabilities_frame()?;
        let frame: &[u8; DEVICE_LIFECYCLE_CAPABILITY_FRAME_BYTES_V1] = frame
            .as_slice()
            .try_into()
            .map_err(|_| DeviceLifecycleBridgeErrorV1::CapabilityMismatch)?;
        let capabilities = decode_capabilities(frame)?;
        if capabilities.platform != expected.platform
            || capabilities.hardware_policy_id != expected.hardware_policy_id
            || capabilities.attestation_digest != expected.attestation_digest
        {
            return Err(DeviceLifecycleBridgeErrorV1::CapabilityMismatch);
        }
        transport
            .authenticate_capabilities(frame, attestation_evidence)
            .map_err(|_| DeviceLifecycleBridgeErrorV1::CapabilityAuthentication)?;
        Ok(Self {
            transport,
            capabilities,
        })
    }

    pub(crate) const fn capabilities(&self) -> &AuthenticatedDeviceLifecycleCapabilitiesV1 {
        &self.capabilities
    }

    fn call<R>(
        &self,
        operation: DeviceLifecycleOperationV1,
        payload: Zeroizing<Vec<u8>>,
        decode: impl FnOnce(&[u8]) -> Result<R, DeviceLifecycleBridgeErrorV1>,
    ) -> Result<R, DeviceLifecycleBridgeErrorV1> {
        let command = encode_command(&self.capabilities, operation, &payload)?;
        let request_id: Digest = command[12..44]
            .try_into()
            .map_err(|_| DeviceLifecycleBridgeErrorV1::MalformedFrame)?;
        let response = self.transport.execute(&command)?;
        let decoded = decode_response(&response, operation, request_id)?;
        if decoded.status != DeviceLifecycleStatusV1::Success {
            return Err(DeviceLifecycleBridgeErrorV1::RemoteStatus(decoded.status));
        }
        let response_authentication_digest = response_authentication_digest(
            &self.capabilities,
            &command,
            decoded.authentication_header,
            decoded.payload,
        );
        self.transport
            .authenticate_response(
                &self.capabilities,
                response_authentication_digest,
                decoded.authenticator,
            )
            .map_err(|_| DeviceLifecycleBridgeErrorV1::ResponseAuthentication)?;
        decode(decoded.payload)
    }
}

impl<T> super::guard::sealed::Sealed for DeviceLifecycleBackendAdapterV1<T> where
    T: AuthenticatedDeviceLifecycleTransportV1
{
}

fn valid_digest(value: Digest) -> bool {
    value != [0; 32]
}

fn sha256(bytes: &[u8]) -> Digest {
    Sha256::digest(bytes).into()
}

fn decode_capabilities(
    frame: &[u8; DEVICE_LIFECYCLE_CAPABILITY_FRAME_BYTES_V1],
) -> Result<AuthenticatedDeviceLifecycleCapabilitiesV1, DeviceLifecycleBridgeErrorV1> {
    let mut reader = Reader::new(frame);
    reader
        .expect(CAPABILITY_MAGIC)
        .map_err(|_| DeviceLifecycleBridgeErrorV1::CapabilityMismatch)?;
    if reader.u16()? != DEVICE_LIFECYCLE_PROTOCOL_VERSION_V1 {
        return Err(DeviceLifecycleBridgeErrorV1::CapabilityMismatch);
    }
    let platform = match reader.u8()? {
        1 => DeviceLifecyclePlatformV1::Android,
        2 => DeviceLifecyclePlatformV1::Ios,
        _ => return Err(DeviceLifecycleBridgeErrorV1::CapabilityMismatch),
    };
    if reader.u8()? != 0
        || reader.u32()? != DEVICE_LIFECYCLE_REQUIRED_CAPABILITY_MASK_V1
        || reader.u32()? as usize != DEVICE_LIFECYCLE_MAX_COMMAND_PAYLOAD_BYTES_V1
        || reader.u32()? as usize != DEVICE_LIFECYCLE_MAX_RESPONSE_PAYLOAD_BYTES_V1
    {
        return Err(DeviceLifecycleBridgeErrorV1::CapabilityMismatch);
    }
    let hardware_policy_id = reader.digest()?;
    let attestation_digest = reader.digest()?;
    if reader.u64()? != 0
        || !reader.is_empty()
        || !valid_digest(hardware_policy_id)
        || !valid_digest(attestation_digest)
        || hardware_policy_id == attestation_digest
    {
        return Err(DeviceLifecycleBridgeErrorV1::CapabilityMismatch);
    }
    Ok(AuthenticatedDeviceLifecycleCapabilitiesV1 {
        platform,
        hardware_policy_id,
        attestation_digest,
    })
}

fn encode_command(
    capabilities: &AuthenticatedDeviceLifecycleCapabilitiesV1,
    operation: DeviceLifecycleOperationV1,
    payload: &[u8],
) -> Result<Zeroizing<Vec<u8>>, DeviceLifecycleBridgeErrorV1> {
    if payload.len() < TYPED_PAYLOAD_HEADER_BYTES
        || payload.len() > DEVICE_LIFECYCLE_MAX_COMMAND_PAYLOAD_BYTES_V1
    {
        return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
    }
    let mut typed = Reader::new(payload);
    typed.expect(COMMAND_PAYLOAD_MAGIC)?;
    if typed.u16()? != DEVICE_LIFECYCLE_PROTOCOL_VERSION_V1
        || typed.u8()? != operation as u8
        || typed.u8()? != 0
    {
        return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
    }
    let operation_byte = [operation as u8];
    let payload_digest = sha256(payload);
    let request_id = digest_framed(
        REQUEST_ID_DOMAIN,
        &[
            &capabilities.hardware_policy_id,
            &capabilities.attestation_digest,
            &operation_byte,
            &payload_digest,
        ],
    );
    if !valid_digest(request_id) {
        return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
    }
    let payload_len = u32::try_from(payload.len())
        .map_err(|_| DeviceLifecycleBridgeErrorV1::InvalidTypedPayload)?;
    let mut output = Zeroizing::new(Vec::with_capacity(
        DEVICE_LIFECYCLE_COMMAND_HEADER_BYTES_V1 + payload.len(),
    ));
    output.extend_from_slice(COMMAND_MAGIC);
    output.extend_from_slice(&DEVICE_LIFECYCLE_PROTOCOL_VERSION_V1.to_le_bytes());
    output.push(operation as u8);
    output.push(0);
    output.extend_from_slice(&request_id);
    output.extend_from_slice(&payload_len.to_le_bytes());
    output.extend_from_slice(&payload_digest);
    output.extend_from_slice(payload);
    Ok(output)
}

struct DecodedResponse<'a> {
    status: DeviceLifecycleStatusV1,
    authentication_header: &'a [u8],
    payload: &'a [u8],
    authenticator: &'a [u8],
}

fn response_authentication_digest(
    capabilities: &AuthenticatedDeviceLifecycleCapabilitiesV1,
    command: &[u8],
    authentication_header: &[u8],
    payload: &[u8],
) -> Digest {
    let platform = [capabilities.platform as u8];
    digest_framed(
        RESPONSE_AUTHENTICATION_DOMAIN,
        &[
            &platform,
            &capabilities.hardware_policy_id,
            &capabilities.attestation_digest,
            command,
            authentication_header,
            payload,
        ],
    )
}

fn decode_response<'a>(
    encoded: &'a [u8],
    expected_operation: DeviceLifecycleOperationV1,
    expected_request_id: Digest,
) -> Result<DecodedResponse<'a>, DeviceLifecycleBridgeErrorV1> {
    if encoded.len() < DEVICE_LIFECYCLE_RESPONSE_HEADER_BYTES_V1
        || encoded.len()
            > DEVICE_LIFECYCLE_RESPONSE_HEADER_BYTES_V1
                + DEVICE_LIFECYCLE_MAX_RESPONSE_PAYLOAD_BYTES_V1
                + DEVICE_LIFECYCLE_MAX_AUTHENTICATOR_BYTES_V1
    {
        return Err(DeviceLifecycleBridgeErrorV1::MalformedFrame);
    }
    let mut reader = Reader::new(encoded);
    reader.expect(RESPONSE_MAGIC)?;
    if reader.u16()? != DEVICE_LIFECYCLE_PROTOCOL_VERSION_V1
        || DeviceLifecycleOperationV1::from_u8(reader.u8()?)? != expected_operation
    {
        return Err(DeviceLifecycleBridgeErrorV1::MalformedFrame);
    }
    let status = DeviceLifecycleStatusV1::from_u8(reader.u8()?)?;
    if reader.digest()? != expected_request_id {
        return Err(DeviceLifecycleBridgeErrorV1::MalformedFrame);
    }
    let payload_len =
        usize::try_from(reader.u32()?).map_err(|_| DeviceLifecycleBridgeErrorV1::MalformedFrame)?;
    let authenticator_len =
        usize::try_from(reader.u32()?).map_err(|_| DeviceLifecycleBridgeErrorV1::MalformedFrame)?;
    if payload_len > DEVICE_LIFECYCLE_MAX_RESPONSE_PAYLOAD_BYTES_V1
        || authenticator_len > DEVICE_LIFECYCLE_MAX_AUTHENTICATOR_BYTES_V1
        || reader.remaining() != 64 + payload_len + authenticator_len
    {
        return Err(DeviceLifecycleBridgeErrorV1::MalformedFrame);
    }
    let payload_digest = reader.digest()?;
    let authenticator_digest = reader.digest()?;
    let authentication_header = &encoded[..RESPONSE_AUTHENTICATOR_DIGEST_OFFSET];
    let payload_start = reader.offset;
    let payload = reader.bytes(payload_len)?;
    let authenticator = reader.bytes(authenticator_len)?;
    if !reader.is_empty()
        || sha256(payload) != payload_digest
        || sha256(authenticator) != authenticator_digest
    {
        return Err(DeviceLifecycleBridgeErrorV1::MalformedFrame);
    }
    if status == DeviceLifecycleStatusV1::Success {
        if payload.is_empty()
            || authenticator.is_empty()
            || !authenticator.iter().any(|byte| *byte != 0)
        {
            return Err(DeviceLifecycleBridgeErrorV1::MalformedFrame);
        }
        let mut typed = Reader::new(payload);
        typed.expect(RESPONSE_PAYLOAD_MAGIC)?;
        if typed.u16()? != DEVICE_LIFECYCLE_PROTOCOL_VERSION_V1
            || typed.u8()? != expected_operation as u8
            || typed.u8()? != 0
        {
            return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
        }
    } else if payload_len != 0 || authenticator_len != 0 {
        return Err(DeviceLifecycleBridgeErrorV1::MalformedFrame);
    }
    debug_assert_eq!(payload_start, DEVICE_LIFECYCLE_RESPONSE_HEADER_BYTES_V1);
    Ok(DecodedResponse {
        status,
        authentication_header,
        payload,
        authenticator,
    })
}

struct Writer {
    bytes: Zeroizing<Vec<u8>>,
}

impl Writer {
    fn command(operation: DeviceLifecycleOperationV1) -> Self {
        let mut bytes = Zeroizing::new(Vec::new());
        bytes.extend_from_slice(COMMAND_PAYLOAD_MAGIC);
        bytes.extend_from_slice(&DEVICE_LIFECYCLE_PROTOCOL_VERSION_V1.to_le_bytes());
        bytes.push(operation as u8);
        bytes.push(0);
        Self { bytes }
    }

    #[cfg(test)]
    fn response(operation: DeviceLifecycleOperationV1) -> Self {
        let mut bytes = Zeroizing::new(Vec::new());
        bytes.extend_from_slice(RESPONSE_PAYLOAD_MAGIC);
        bytes.extend_from_slice(&DEVICE_LIFECYCLE_PROTOCOL_VERSION_V1.to_le_bytes());
        bytes.push(operation as u8);
        bytes.push(0);
        Self { bytes }
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn digest(&mut self, value: Digest) {
        self.bytes.extend_from_slice(&value);
    }

    fn optional_digest(&mut self, value: Option<Digest>) {
        match value {
            Some(value) => {
                self.u8(1);
                self.digest(value);
            }
            None => {
                self.u8(0);
                self.digest([0; 32]);
            }
        }
    }

    fn bounded_bytes(
        &mut self,
        value: &[u8],
        maximum: usize,
    ) -> Result<(), DeviceLifecycleBridgeErrorV1> {
        if value.is_empty() || value.len() > maximum {
            return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
        }
        self.u32(
            u32::try_from(value.len())
                .map_err(|_| DeviceLifecycleBridgeErrorV1::InvalidTypedPayload)?,
        );
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn finish(self) -> Result<Zeroizing<Vec<u8>>, DeviceLifecycleBridgeErrorV1> {
        if self.bytes.len() < TYPED_PAYLOAD_HEADER_BYTES
            || self.bytes.len() > DEVICE_LIFECYCLE_MAX_COMMAND_PAYLOAD_BYTES_V1
        {
            return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
        }
        Ok(self.bytes)
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn bytes(&mut self, count: usize) -> Result<&'a [u8], DeviceLifecycleBridgeErrorV1> {
        let end = self
            .offset
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(DeviceLifecycleBridgeErrorV1::MalformedFrame)?;
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }

    fn expect(&mut self, expected: &[u8]) -> Result<(), DeviceLifecycleBridgeErrorV1> {
        if self.bytes(expected.len())? != expected {
            return Err(DeviceLifecycleBridgeErrorV1::MalformedFrame);
        }
        Ok(())
    }

    fn u8(&mut self) -> Result<u8, DeviceLifecycleBridgeErrorV1> {
        self.bytes(1)?
            .first()
            .copied()
            .ok_or(DeviceLifecycleBridgeErrorV1::MalformedFrame)
    }

    fn u16(&mut self) -> Result<u16, DeviceLifecycleBridgeErrorV1> {
        let bytes: [u8; 2] = self
            .bytes(2)?
            .try_into()
            .map_err(|_| DeviceLifecycleBridgeErrorV1::MalformedFrame)?;
        Ok(u16::from_le_bytes(bytes))
    }

    fn u32(&mut self) -> Result<u32, DeviceLifecycleBridgeErrorV1> {
        let bytes: [u8; 4] = self
            .bytes(4)?
            .try_into()
            .map_err(|_| DeviceLifecycleBridgeErrorV1::MalformedFrame)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64, DeviceLifecycleBridgeErrorV1> {
        let bytes: [u8; 8] = self
            .bytes(8)?
            .try_into()
            .map_err(|_| DeviceLifecycleBridgeErrorV1::MalformedFrame)?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn digest(&mut self) -> Result<Digest, DeviceLifecycleBridgeErrorV1> {
        self.bytes(32)?
            .try_into()
            .map_err(|_| DeviceLifecycleBridgeErrorV1::MalformedFrame)
    }

    fn optional_digest(&mut self) -> Result<Option<Digest>, DeviceLifecycleBridgeErrorV1> {
        let present = self.u8()?;
        let digest = self.digest()?;
        match (present, digest) {
            (0, digest) if digest == [0; 32] => Ok(None),
            (1, digest) if valid_digest(digest) => Ok(Some(digest)),
            _ => Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload),
        }
    }

    fn bounded_bytes(&mut self, maximum: usize) -> Result<&'a [u8], DeviceLifecycleBridgeErrorV1> {
        let count = usize::try_from(self.u32()?)
            .map_err(|_| DeviceLifecycleBridgeErrorV1::InvalidTypedPayload)?;
        if count == 0 || count > maximum {
            return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
        }
        self.bytes(count)
    }
}

fn begin_response_payload(
    payload: &[u8],
    operation: DeviceLifecycleOperationV1,
) -> Result<Reader<'_>, DeviceLifecycleBridgeErrorV1> {
    let mut reader = Reader::new(payload);
    reader.expect(RESPONSE_PAYLOAD_MAGIC)?;
    if reader.u16()? != DEVICE_LIFECYCLE_PROTOCOL_VERSION_V1
        || reader.u8()? != operation as u8
        || reader.u8()? != 0
    {
        return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
    }
    Ok(reader)
}

fn encode_intent(writer: &mut Writer, request: &HardwareIntentRequestV1) {
    writer.u8(request.kind() as u8);
    writer.digest(request.device_id());
    writer.digest(request.hardware_policy_id());
    writer.digest(request.wallet_binding());
    writer.digest(request.context_digest());
    writer.digest(request.current_head());
    writer.digest(request.current_lineage_digest());
    writer.u64(request.from_sequence());
    writer.u64(request.not_before_ms());
    writer.u64(request.expires_at_ms());
    writer.digest(request.intent_digest());
    writer.digest(request.challenge_digest());
}

fn decode_intent(
    reader: &mut Reader<'_>,
) -> Result<HardwareIntentRequestV1, DeviceLifecycleBridgeErrorV1> {
    let kind = match reader.u8()? {
        1 => HardwareIntentKindV1::ReceivePending,
        2 => HardwareIntentKindV1::SendPublished,
        _ => return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload),
    };
    let device_id = reader.digest()?;
    let hardware_policy_id = reader.digest()?;
    let wallet_binding = reader.digest()?;
    let context_digest = reader.digest()?;
    let current_head = reader.digest()?;
    let current_lineage_digest = reader.digest()?;
    let from_sequence = reader.u64()?;
    let not_before_ms = reader.u64()?;
    let expires_at_ms = reader.u64()?;
    let intent_digest = reader.digest()?;
    let challenge_digest = reader.digest()?;
    let required = [
        device_id,
        hardware_policy_id,
        wallet_binding,
        context_digest,
        current_head,
        current_lineage_digest,
        intent_digest,
        challenge_digest,
    ];
    if required.into_iter().any(|digest| !valid_digest(digest)) || not_before_ms >= expires_at_ms {
        return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
    }
    let kind_byte = [kind as u8];
    let expected = digest_framed(
        INTENT_CHALLENGE_DOMAIN,
        &[
            &kind_byte,
            &device_id,
            &hardware_policy_id,
            &wallet_binding,
            &context_digest,
            &current_head,
            &current_lineage_digest,
            &from_sequence.to_le_bytes(),
            &not_before_ms.to_le_bytes(),
            &expires_at_ms.to_le_bytes(),
            &intent_digest,
        ],
    );
    if expected != challenge_digest {
        return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
    }
    Ok(HardwareIntentChallengeV1 {
        kind,
        device_id,
        hardware_policy_id,
        wallet_binding,
        context_digest,
        current_head,
        current_lineage_digest,
        from_sequence,
        not_before_ms,
        expires_at_ms,
        intent_digest,
        digest: challenge_digest,
    }
    .hardware_request())
}

fn encode_guard_request(writer: &mut Writer, request: &HardwareGuardRequestV1) {
    writer.digest(request.device_id());
    writer.digest(request.hardware_policy_id());
    writer.digest(request.wallet_binding());
    writer.u64(request.from_sequence());
    writer.u64(request.to_sequence());
    writer.digest(request.challenge_digest());
}

fn encode_outbox_key(writer: &mut Writer, key: &PaymentOutboxKeyV1) {
    writer.digest(key.device_id());
    writer.digest(key.hardware_policy_id());
    writer.digest(key.wallet_binding());
    writer.digest(key.context_digest());
    writer.digest(key.request_digest());
    writer.digest(key.send_transition_digest());
    writer.digest(key.guard_challenge_digest());
    writer.digest(key.digest());
}

fn expect_outbox_key(
    reader: &mut Reader<'_>,
    expected: &PaymentOutboxKeyV1,
) -> Result<(), DeviceLifecycleBridgeErrorV1> {
    let fields = [
        expected.device_id(),
        expected.hardware_policy_id(),
        expected.wallet_binding(),
        expected.context_digest(),
        expected.request_digest(),
        expected.send_transition_digest(),
        expected.guard_challenge_digest(),
        expected.digest(),
    ];
    for field in fields {
        if reader.digest()? != field || !valid_digest(field) {
            return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
        }
    }
    Ok(())
}

fn encode_terminal(writer: &mut Writer, outcome: &HardwareTerminalOutcomeV1) {
    writer.u8(outcome.operation() as u8);
    encode_intent(writer, outcome.intent());
    writer.u64(outcome.intent_epoch());
    writer.u64(outcome.from_sequence());
    writer.u64(outcome.to_sequence());
    writer.digest(outcome.intent_binding_digest());
    writer.digest(outcome.completion_digest());
    writer.optional_digest(outcome.payment_digest());
    writer.optional_digest(outcome.acknowledgement_digest());
    writer.u64(outcome.trusted_time_ms());
    writer.digest(outcome.successor_head());
}

fn decode_terminal(
    reader: &mut Reader<'_>,
) -> Result<HardwareTerminalOutcomeV1, DeviceLifecycleBridgeErrorV1> {
    let operation = match reader.u8()? {
        1 => HardwareTerminalOperationV1::ReceiveCancelled,
        2 => HardwareTerminalOperationV1::SendCommitted,
        3 => HardwareTerminalOperationV1::ReceiveCommitted,
        _ => return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload),
    };
    let intent = decode_intent(reader)?;
    let intent_epoch = reader.u64()?;
    let from_sequence = reader.u64()?;
    let to_sequence = reader.u64()?;
    let intent_binding_digest = reader.digest()?;
    let completion_digest = reader.digest()?;
    let payment_digest = reader.optional_digest()?;
    let acknowledgement_digest = reader.optional_digest()?;
    let trusted_time_ms = reader.u64()?;
    let successor_head = reader.digest()?;
    let exact_next = from_sequence
        .checked_add(1)
        .is_some_and(|next| next == to_sequence);
    let shape_valid = match operation {
        HardwareTerminalOperationV1::ReceiveCancelled => {
            intent.kind() == HardwareIntentKindV1::ReceivePending
                && from_sequence == to_sequence
                && payment_digest.is_none()
                && acknowledgement_digest.is_none()
                && successor_head == intent.current_head()
        }
        HardwareTerminalOperationV1::SendCommitted => {
            intent.kind() == HardwareIntentKindV1::SendPublished
                && exact_next
                && payment_digest.is_some()
                && acknowledgement_digest.is_some()
        }
        HardwareTerminalOperationV1::ReceiveCommitted => {
            intent.kind() == HardwareIntentKindV1::ReceivePending
                && exact_next
                && payment_digest.is_some()
        }
    };
    if !shape_valid
        || intent_epoch == 0
        || from_sequence != intent.from_sequence()
        || !valid_digest(intent_binding_digest)
        || !valid_digest(completion_digest)
        || !valid_digest(successor_head)
    {
        return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
    }
    Ok(HardwareTerminalOutcomeV1::new(
        operation,
        intent,
        intent_epoch,
        from_sequence,
        to_sequence,
        intent_binding_digest,
        completion_digest,
        payment_digest,
        acknowledgement_digest,
        trusted_time_ms,
        successor_head,
    ))
}

fn require_end(reader: &Reader<'_>) -> Result<(), DeviceLifecycleBridgeErrorV1> {
    if reader.is_empty() {
        Ok(())
    } else {
        Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload)
    }
}

fn validate_intent_request(
    request: &HardwareIntentRequestV1,
) -> Result<(), DeviceLifecycleBridgeErrorV1> {
    let required = [
        request.device_id(),
        request.hardware_policy_id(),
        request.wallet_binding(),
        request.context_digest(),
        request.current_head(),
        request.current_lineage_digest(),
        request.intent_digest(),
        request.challenge_digest(),
    ];
    if required.into_iter().any(|digest| !valid_digest(digest))
        || request.not_before_ms() >= request.expires_at_ms()
    {
        return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
    }
    let kind_byte = [request.kind() as u8];
    let expected = digest_framed(
        INTENT_CHALLENGE_DOMAIN,
        &[
            &kind_byte,
            &request.device_id(),
            &request.hardware_policy_id(),
            &request.wallet_binding(),
            &request.context_digest(),
            &request.current_head(),
            &request.current_lineage_digest(),
            &request.from_sequence().to_le_bytes(),
            &request.not_before_ms().to_le_bytes(),
            &request.expires_at_ms().to_le_bytes(),
            &request.intent_digest(),
        ],
    );
    if expected != request.challenge_digest() {
        return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
    }
    Ok(())
}

fn validate_outbox_key(key: &PaymentOutboxKeyV1) -> Result<(), DeviceLifecycleBridgeErrorV1> {
    let required = [
        key.device_id(),
        key.hardware_policy_id(),
        key.wallet_binding(),
        key.context_digest(),
        key.request_digest(),
        key.send_transition_digest(),
        key.guard_challenge_digest(),
        key.digest(),
    ];
    if required.into_iter().any(|digest| !valid_digest(digest)) {
        return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
    }
    Ok(())
}

fn encode_guard_command(
    operation: DeviceLifecycleOperationV1,
    request: &HardwareIntentRequestV1,
) -> Result<Writer, DeviceLifecycleBridgeErrorV1> {
    validate_intent_request(request)?;
    let mut writer = Writer::command(operation);
    encode_intent(&mut writer, request);
    Ok(writer)
}

impl<T> ExactNextHardwareGuardBackendV1 for DeviceLifecycleBackendAdapterV1<T>
where
    T: AuthenticatedDeviceLifecycleTransportV1,
{
    fn reserve_receive_intent_and_sign_or_recover(
        &self,
        request: &HardwareIntentRequestV1,
        signing_bytes: &[u8],
        receiver_public_key: &KagemushaDevicePublicKeyV2,
    ) -> Result<HardwareReceiveSigningResultV1, HardwareGuardErrorV1> {
        let operation = DeviceLifecycleOperationV1::ReserveReceiveIntentAndSign;
        let result = (|| {
            if request.kind() != HardwareIntentKindV1::ReceivePending {
                return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
            }
            let mut writer = encode_guard_command(operation, request)?;
            // Keep the canonical signing preimage opaque. Core binds every
            // request field here without freezing the data-model layout in the
            // device ABI; separately typed device keys follow this byte string.
            writer.bounded_bytes(signing_bytes, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1)?;
            writer
                .bytes
                .extend_from_slice(receiver_public_key.as_sec1_bytes());
            let payload = writer.finish()?;
            self.call(operation, payload, |payload| {
                let mut reader = begin_response_payload(payload, operation)?;
                let epoch = reader.u64()?;
                let signature = KagemushaDeviceSignatureV2::from_raw_bytes(
                    reader.bytes(KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2)?,
                )
                .map_err(|_| DeviceLifecycleBridgeErrorV1::InvalidTypedPayload)?;
                if epoch == 0 {
                    return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
                }
                require_end(&reader)?;
                Ok(HardwareReceiveSigningResultV1::new(epoch, signature))
            })
        })();
        result.map_err(guard_error)
    }

    fn recover_receive_intent_and_signature(
        &self,
        request: &HardwareIntentRequestV1,
        signing_bytes: &[u8],
        receiver_public_key: &KagemushaDevicePublicKeyV2,
    ) -> Result<HardwareReceiveSigningResultV1, HardwareGuardErrorV1> {
        let operation = DeviceLifecycleOperationV1::RecoverReceiveIntentAndSignature;
        let result = (|| {
            if request.kind() != HardwareIntentKindV1::ReceivePending {
                return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
            }
            let mut writer = encode_guard_command(operation, request)?;
            // See the reserve path: do not parse or truncate the canonical
            // request signing preimage in this adapter.
            writer.bounded_bytes(signing_bytes, OFFLINE_CASH_PAYMENT_REQUEST_MAX_BYTES_V1)?;
            writer
                .bytes
                .extend_from_slice(receiver_public_key.as_sec1_bytes());
            let payload = writer.finish()?;
            self.call(operation, payload, |payload| {
                let mut reader = begin_response_payload(payload, operation)?;
                let epoch = reader.u64()?;
                let signature = KagemushaDeviceSignatureV2::from_raw_bytes(
                    reader.bytes(KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2)?,
                )
                .map_err(|_| DeviceLifecycleBridgeErrorV1::InvalidTypedPayload)?;
                if epoch == 0 {
                    return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
                }
                require_end(&reader)?;
                Ok(HardwareReceiveSigningResultV1::new(epoch, signature))
            })
        })();
        result.map_err(guard_error)
    }

    fn bind_receive_request_digest_or_recover(
        &self,
        request: &HardwareIntentRequestV1,
        intent_epoch: u64,
        request_digest: Digest,
    ) -> Result<(), HardwareGuardErrorV1> {
        let operation = DeviceLifecycleOperationV1::BindReceiveRequestDigest;
        let result = (|| {
            if request.kind() != HardwareIntentKindV1::ReceivePending
                || intent_epoch == 0
                || !valid_digest(request_digest)
            {
                return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
            }
            let mut writer = encode_guard_command(operation, request)?;
            writer.u64(intent_epoch);
            writer.digest(request_digest);
            let payload = writer.finish()?;
            self.call(operation, payload, |payload| {
                let mut reader = begin_response_payload(payload, operation)?;
                if reader.digest()? != request_digest {
                    return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
                }
                require_end(&reader)
            })
        })();
        result.map_err(guard_error)
    }

    fn publish_send_payment_or_recover(
        &self,
        request: &HardwareIntentRequestV1,
        payment_digest: Digest,
    ) -> Result<u64, HardwareGuardErrorV1> {
        let operation = DeviceLifecycleOperationV1::PublishSendPayment;
        let result = (|| {
            if request.kind() != HardwareIntentKindV1::SendPublished
                || !valid_digest(payment_digest)
            {
                return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
            }
            let mut writer = encode_guard_command(operation, request)?;
            writer.digest(payment_digest);
            let payload = writer.finish()?;
            self.call(operation, payload, |payload| {
                let mut reader = begin_response_payload(payload, operation)?;
                let epoch = reader.u64()?;
                if epoch == 0 || reader.digest()? != payment_digest {
                    return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
                }
                require_end(&reader)?;
                Ok(epoch)
            })
        })();
        result.map_err(guard_error)
    }

    fn recover_active_intent(
        &self,
        request: &HardwareIntentRequestV1,
    ) -> Result<HardwareActiveIntentOutcomeV1, HardwareGuardErrorV1> {
        let operation = DeviceLifecycleOperationV1::RecoverActiveIntent;
        let result = (|| {
            let writer = encode_guard_command(operation, request)?;
            let payload = writer.finish()?;
            self.call(operation, payload, |payload| {
                let mut reader = begin_response_payload(payload, operation)?;
                if decode_intent(&mut reader)? != *request {
                    return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
                }
                let epoch = reader.u64()?;
                let bound_digest = reader.digest()?;
                if epoch == 0 || !valid_digest(bound_digest) {
                    return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
                }
                require_end(&reader)?;
                Ok(HardwareActiveIntentOutcomeV1::new(
                    *request,
                    epoch,
                    bound_digest,
                ))
            })
        })();
        result.map_err(guard_error)
    }

    fn cancel_expired_receive_or_recover(
        &self,
        request: &HardwareIntentRequestV1,
        intent_epoch: u64,
        completion_digest: Digest,
    ) -> Result<HardwareTerminalOutcomeV1, HardwareGuardErrorV1> {
        let operation = DeviceLifecycleOperationV1::CancelExpiredReceive;
        let result = (|| {
            if request.kind() != HardwareIntentKindV1::ReceivePending
                || intent_epoch == 0
                || !valid_digest(completion_digest)
            {
                return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
            }
            let mut writer = encode_guard_command(operation, request)?;
            writer.u64(intent_epoch);
            writer.digest(completion_digest);
            let payload = writer.finish()?;
            self.call(operation, payload, |payload| {
                let mut reader = begin_response_payload(payload, operation)?;
                let outcome = decode_terminal(&mut reader)?;
                if outcome.operation() != HardwareTerminalOperationV1::ReceiveCancelled
                    || outcome.intent() != request
                    || outcome.intent_epoch() != intent_epoch
                    || outcome.completion_digest() != completion_digest
                {
                    return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
                }
                require_end(&reader)?;
                Ok(outcome)
            })
        })();
        result.map_err(guard_error)
    }

    fn commit_intent_or_recover_exact_next(
        &self,
        request: &HardwareIntentCommitRequestV1,
    ) -> Result<HardwareTerminalOutcomeV1, HardwareGuardErrorV1> {
        let operation = DeviceLifecycleOperationV1::CommitIntentExactNext;
        let result = (|| {
            validate_intent_request(request.intent())?;
            let guard = request.guard();
            if request.intent_epoch() == 0
                || !valid_digest(request.intent_binding_digest())
                || !valid_digest(request.payment_digest())
                || !valid_digest(request.completion_digest())
                || !valid_digest(request.successor_head())
                || !valid_digest(guard.device_id())
                || !valid_digest(guard.hardware_policy_id())
                || !valid_digest(guard.wallet_binding())
                || !valid_digest(guard.challenge_digest())
                || guard.from_sequence().checked_add(1) != Some(guard.to_sequence())
            {
                return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
            }
            let mut writer = Writer::command(operation);
            encode_intent(&mut writer, request.intent());
            writer.u64(request.intent_epoch());
            writer.digest(request.intent_binding_digest());
            encode_guard_request(&mut writer, guard);
            writer.digest(request.payment_digest());
            writer.optional_digest(request.acknowledgement_digest());
            writer.digest(request.completion_digest());
            writer.digest(request.successor_head());
            let payload = writer.finish()?;
            self.call(operation, payload, |payload| {
                let mut reader = begin_response_payload(payload, operation)?;
                let outcome = decode_terminal(&mut reader)?;
                if outcome.intent() != request.intent()
                    || outcome.intent_epoch() != request.intent_epoch()
                    || outcome.from_sequence() != guard.from_sequence()
                    || outcome.to_sequence() != guard.to_sequence()
                    || outcome.intent_binding_digest() != request.intent_binding_digest()
                    || outcome.payment_digest() != Some(request.payment_digest())
                    || outcome.acknowledgement_digest() != request.acknowledgement_digest()
                    || outcome.completion_digest() != request.completion_digest()
                    || outcome.successor_head() != request.successor_head()
                {
                    return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
                }
                require_end(&reader)?;
                Ok(outcome)
            })
        })();
        result.map_err(guard_error)
    }

    fn recover_terminal_outcome(
        &self,
        request: &HardwareIntentRequestV1,
    ) -> Result<HardwareTerminalOutcomeV1, HardwareGuardErrorV1> {
        let operation = DeviceLifecycleOperationV1::RecoverTerminal;
        let result = (|| {
            let writer = encode_guard_command(operation, request)?;
            let payload = writer.finish()?;
            self.call(operation, payload, |payload| {
                let mut reader = begin_response_payload(payload, operation)?;
                let outcome = decode_terminal(&mut reader)?;
                if outcome.intent() != request {
                    return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
                }
                require_end(&reader)?;
                Ok(outcome)
            })
        })();
        result.map_err(guard_error)
    }

    fn recover_receive_terminal_outcome(
        &self,
        query: &HardwareReceiveTerminalQueryV1,
    ) -> Result<HardwareTerminalOutcomeV1, HardwareGuardErrorV1> {
        let operation = DeviceLifecycleOperationV1::RecoverReceiveTerminal;
        let result = (|| {
            let fields = [
                query.device_id(),
                query.hardware_policy_id(),
                query.wallet_binding(),
                query.context_digest(),
                query.request_digest(),
                query.payment_digest(),
            ];
            if fields.into_iter().any(|digest| !valid_digest(digest)) {
                return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
            }
            let mut writer = Writer::command(operation);
            for field in fields {
                writer.digest(field);
            }
            let payload = writer.finish()?;
            self.call(operation, payload, |payload| {
                let mut reader = begin_response_payload(payload, operation)?;
                let outcome = decode_terminal(&mut reader)?;
                if outcome.operation() != HardwareTerminalOperationV1::ReceiveCommitted
                    || outcome.intent().device_id() != query.device_id()
                    || outcome.intent().hardware_policy_id() != query.hardware_policy_id()
                    || outcome.intent().wallet_binding() != query.wallet_binding()
                    || outcome.intent().context_digest() != query.context_digest()
                    || outcome.intent_binding_digest() != query.request_digest()
                    || outcome.payment_digest() != Some(query.payment_digest())
                {
                    return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
                }
                require_end(&reader)?;
                Ok(outcome)
            })
        })();
        result.map_err(guard_error)
    }

    fn sign_receive_acknowledgement_or_recover(
        &self,
        outcome: &HardwareTerminalOutcomeV1,
        acknowledgement_digest: Digest,
        signing_bytes: &[u8],
        receiver_public_key: &KagemushaDevicePublicKeyV2,
    ) -> Result<KagemushaDeviceSignatureV2, HardwareGuardErrorV1> {
        let operation = DeviceLifecycleOperationV1::SignReceiveAcknowledgement;
        let result = (|| {
            if outcome.operation() != HardwareTerminalOperationV1::ReceiveCommitted
                || !valid_digest(acknowledgement_digest)
            {
                return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
            }
            let mut writer = Writer::command(operation);
            encode_terminal(&mut writer, outcome);
            writer.digest(acknowledgement_digest);
            writer.bounded_bytes(signing_bytes, OFFLINE_CASH_ACKNOWLEDGEMENT_MAX_BYTES_V1)?;
            writer
                .bytes
                .extend_from_slice(receiver_public_key.as_sec1_bytes());
            let payload = writer.finish()?;
            self.call(operation, payload, |payload| {
                let mut reader = begin_response_payload(payload, operation)?;
                if reader.digest()? != acknowledgement_digest {
                    return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
                }
                let signature = KagemushaDeviceSignatureV2::from_raw_bytes(
                    reader.bytes(KAGEMUSHA_DEVICE_SIGNATURE_BYTES_V2)?,
                )
                .map_err(|_| DeviceLifecycleBridgeErrorV1::InvalidTypedPayload)?;
                require_end(&reader)?;
                Ok(signature)
            })
        })();
        result.map_err(guard_error)
    }
}

impl<T> AuthenticatedPaymentOutboxBackendV1 for DeviceLifecycleBackendAdapterV1<T>
where
    T: AuthenticatedDeviceLifecycleTransportV1,
{
    fn stage_payment_or_recover(
        &self,
        key: &PaymentOutboxKeyV1,
        payment_digest: Digest,
        canonical_payment: &[u8],
    ) -> Result<(), AuthenticatedPaymentOutboxErrorV1> {
        let operation = DeviceLifecycleOperationV1::StagePayment;
        let result = (|| {
            validate_outbox_key(key)?;
            if !valid_digest(payment_digest) {
                return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
            }
            let mut writer = Writer::command(operation);
            encode_outbox_key(&mut writer, key);
            writer.digest(payment_digest);
            writer.bounded_bytes(canonical_payment, OFFLINE_CASH_PAYMENT_MAX_BYTES_V1)?;
            let payload = writer.finish()?;
            self.call(operation, payload, |payload| {
                let mut reader = begin_response_payload(payload, operation)?;
                expect_outbox_key(&mut reader, key)?;
                if reader.digest()? != payment_digest {
                    return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
                }
                require_end(&reader)
            })
        })();
        result.map_err(outbox_error)
    }

    fn recover_staged_payment_digest(
        &self,
        key: &PaymentOutboxKeyV1,
    ) -> Result<Digest, AuthenticatedPaymentOutboxErrorV1> {
        let operation = DeviceLifecycleOperationV1::RecoverStagedPaymentDigest;
        let result = (|| {
            validate_outbox_key(key)?;
            let mut writer = Writer::command(operation);
            encode_outbox_key(&mut writer, key);
            let payload = writer.finish()?;
            self.call(operation, payload, |payload| {
                let mut reader = begin_response_payload(payload, operation)?;
                expect_outbox_key(&mut reader, key)?;
                let payment_digest = reader.digest()?;
                if !valid_digest(payment_digest) {
                    return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
                }
                require_end(&reader)?;
                Ok(payment_digest)
            })
        })();
        result.map_err(outbox_error)
    }

    fn publish_payment_or_recover(
        &self,
        authorization: &PaymentOutboxPublicationV1,
    ) -> Result<AuthenticatedPaymentOutboxRecordV1, AuthenticatedPaymentOutboxErrorV1> {
        let operation = DeviceLifecycleOperationV1::PublishStagedPayment;
        let result = (|| {
            let key = authorization.key();
            validate_outbox_key(key)?;
            if !valid_digest(authorization.payment_digest())
                || authorization.intent_epoch() == 0
                || !valid_digest(authorization.intent_digest())
                || !valid_digest(authorization.authorization_digest())
            {
                return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
            }
            let mut writer = Writer::command(operation);
            encode_outbox_key(&mut writer, key);
            writer.digest(authorization.payment_digest());
            writer.u64(authorization.intent_epoch());
            writer.digest(authorization.intent_digest());
            writer.digest(authorization.authorization_digest());
            let payload = writer.finish()?;
            self.call(operation, payload, |payload| {
                decode_outbox_record(
                    payload,
                    operation,
                    key,
                    Some(authorization.payment_digest()),
                    Some(authorization.authorization_digest()),
                )
            })
        })();
        result.map_err(outbox_error)
    }

    fn recover_published_payment(
        &self,
        key: &PaymentOutboxKeyV1,
    ) -> Result<AuthenticatedPaymentOutboxRecordV1, AuthenticatedPaymentOutboxErrorV1> {
        let operation = DeviceLifecycleOperationV1::RecoverPublishedPayment;
        let result = (|| {
            validate_outbox_key(key)?;
            let mut writer = Writer::command(operation);
            encode_outbox_key(&mut writer, key);
            let payload = writer.finish()?;
            self.call(operation, payload, |payload| {
                decode_outbox_record(payload, operation, key, None, None)
            })
        })();
        result.map_err(outbox_error)
    }
}

fn decode_outbox_record(
    payload: &[u8],
    operation: DeviceLifecycleOperationV1,
    expected_key: &PaymentOutboxKeyV1,
    expected_payment_digest: Option<Digest>,
    expected_publication_digest: Option<Digest>,
) -> Result<AuthenticatedPaymentOutboxRecordV1, DeviceLifecycleBridgeErrorV1> {
    let mut reader = begin_response_payload(payload, operation)?;
    expect_outbox_key(&mut reader, expected_key)?;
    let payment_digest = reader.digest()?;
    let canonical_payment = reader.bounded_bytes(OFFLINE_CASH_PAYMENT_MAX_BYTES_V1)?;
    let publication_digest = reader.optional_digest()?;
    if !valid_digest(payment_digest)
        || expected_payment_digest.is_some_and(|expected| expected != payment_digest)
        || publication_digest.is_none()
        || expected_publication_digest.is_some_and(|expected| publication_digest != Some(expected))
    {
        return Err(DeviceLifecycleBridgeErrorV1::InvalidTypedPayload);
    }
    require_end(&reader)?;
    Ok(AuthenticatedPaymentOutboxRecordV1::new(
        *expected_key,
        payment_digest,
        canonical_payment.to_vec(),
        publication_digest,
    ))
}

fn guard_error(error: DeviceLifecycleBridgeErrorV1) -> HardwareGuardErrorV1 {
    match error {
        DeviceLifecycleBridgeErrorV1::Unavailable => HardwareGuardErrorV1::Unavailable,
        DeviceLifecycleBridgeErrorV1::RemoteStatus(status) => match status {
            DeviceLifecycleStatusV1::Unavailable => HardwareGuardErrorV1::Unavailable,
            DeviceLifecycleStatusV1::StaleOrConcurrent | DeviceLifecycleStatusV1::Conflict => {
                HardwareGuardErrorV1::StaleOrConcurrent
            }
            DeviceLifecycleStatusV1::IntentMismatch | DeviceLifecycleStatusV1::Missing => {
                HardwareGuardErrorV1::IntentMismatch
            }
            DeviceLifecycleStatusV1::TrustedTimeRejected => {
                HardwareGuardErrorV1::TrustedTimeRejected
            }
            DeviceLifecycleStatusV1::PolicyRejected
            | DeviceLifecycleStatusV1::Corrupt
            | DeviceLifecycleStatusV1::MalformedRequest
            | DeviceLifecycleStatusV1::Success => HardwareGuardErrorV1::PolicyRejected,
        },
        DeviceLifecycleBridgeErrorV1::CapabilityMismatch
        | DeviceLifecycleBridgeErrorV1::CapabilityAuthentication
        | DeviceLifecycleBridgeErrorV1::InvalidTypedPayload
        | DeviceLifecycleBridgeErrorV1::MalformedFrame
        | DeviceLifecycleBridgeErrorV1::ResponseAuthentication => {
            HardwareGuardErrorV1::PolicyRejected
        }
    }
}

fn outbox_error(error: DeviceLifecycleBridgeErrorV1) -> AuthenticatedPaymentOutboxErrorV1 {
    match error {
        DeviceLifecycleBridgeErrorV1::Unavailable => AuthenticatedPaymentOutboxErrorV1::Unavailable,
        DeviceLifecycleBridgeErrorV1::RemoteStatus(status) => match status {
            DeviceLifecycleStatusV1::Unavailable => AuthenticatedPaymentOutboxErrorV1::Unavailable,
            DeviceLifecycleStatusV1::IntentMismatch | DeviceLifecycleStatusV1::Missing => {
                AuthenticatedPaymentOutboxErrorV1::Missing
            }
            DeviceLifecycleStatusV1::StaleOrConcurrent | DeviceLifecycleStatusV1::Conflict => {
                AuthenticatedPaymentOutboxErrorV1::Conflict
            }
            DeviceLifecycleStatusV1::TrustedTimeRejected
            | DeviceLifecycleStatusV1::PolicyRejected
            | DeviceLifecycleStatusV1::Corrupt
            | DeviceLifecycleStatusV1::MalformedRequest
            | DeviceLifecycleStatusV1::Success => AuthenticatedPaymentOutboxErrorV1::Corrupt,
        },
        DeviceLifecycleBridgeErrorV1::CapabilityMismatch
        | DeviceLifecycleBridgeErrorV1::CapabilityAuthentication
        | DeviceLifecycleBridgeErrorV1::InvalidTypedPayload
        | DeviceLifecycleBridgeErrorV1::MalformedFrame
        | DeviceLifecycleBridgeErrorV1::ResponseAuthentication => {
            AuthenticatedPaymentOutboxErrorV1::Corrupt
        }
    }
}

#[cfg(test)]
#[path = "device_bridge_tests.rs"]
mod tests;
