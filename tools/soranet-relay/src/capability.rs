//! Capability negotiation helpers for the SoraNet relay handshake.
//!
//! This module implements the TLV parsing and negotiation rules documented in
//! `specs/soranet_handshake.md`. It is intentionally deterministic so
//! transcript hashes computed by clients and relays match byte-for-byte.
use crate::{config::PaddingConfig, constant_rate::CONSTANT_RATE_CELL_BYTES};
use hex::FromHex;
pub use iroha_crypto::soranet::handshake::{
    CapabilityWarning, MAX_CAPABILITY_VECTOR_LEN as MAX_CAP_VECTOR_LEN,
};
use std::fmt;
use thiserror::Error;
/// `snnet.pqkem` TLV type.
pub const TYPE_PQ_KEM: u16 = 0x0101;
/// `snnet.pqsig` TLV type.
pub const TYPE_PQ_SIG: u16 = 0x0102;
/// `snnet.transcript_commit` TLV type.
pub const TYPE_TRANSCRIPT_COMMIT: u16 = 0x0103;
/// `snnet.suite_list` TLV type.
pub const TYPE_SUITE_LIST: u16 = 0x0104;
/// `snnet.role` TLV type.
pub const TYPE_ROLE: u16 = 0x0201;
/// `snnet.padding` TLV type.
pub const TYPE_PADDING: u16 = 0x0202;
/// `snnet.constant_rate` TLV type.
pub const TYPE_CONSTANT_RATE: u16 = 0x0203;
const REQUIRED_FLAG: u8 = 0x01;
const CONSTANT_RATE_FLAG_STRICT: u8 = 0x01;
const SINGLETON_TRANSCRIPT_COMMIT: u8 = 1 << 0;
const SINGLETON_SUITE_LIST: u8 = 1 << 1;
const SINGLETON_ROLE: u8 = 1 << 2;
const SINGLETON_PADDING: u8 = 1 << 3;
const SINGLETON_CONSTANT_RATE: u8 = 1 << 4;
/// Recognised ML-KEM variants exchanged during capability negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KemId {
    /// ML-KEM-512.
    MlKem512,
    /// Kyber768 (ML-KEM-768).
    MlKem768,
    /// Kyber1024 (ML-KEM-1024).
    MlKem1024,
}
impl KemId {
    /// Return the wire code associated with this KEM identifier.
    pub const fn code(self) -> u8 {
        match self {
            Self::MlKem512 => 0x00,
            Self::MlKem768 => 0x01,
            Self::MlKem1024 => 0x02,
        }
    }
    /// Convert a wire code into a [`KemId`], rejecting unknown codes.
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            0x00 => Some(Self::MlKem512),
            0x01 => Some(Self::MlKem768),
            0x02 => Some(Self::MlKem1024),
            _ => None,
        }
    }
}
/// Signature variants accepted during capability negotiation.
///
/// SNNet-16 v1 defines a single transcript signature identifier. Online relay
/// identity authentication remains a separate Ed25519 operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureId {
    /// Dilithium3 (the SNNet-16 v1 transcript signature).
    Dilithium3,
}
impl SignatureId {
    /// Return the wire code associated with this signature identifier.
    pub const fn code(self) -> u8 {
        match self {
            Self::Dilithium3 => 0x01,
        }
    }
    /// Convert a wire code into a [`SignatureId`], rejecting unknown codes.
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            0x01 => Some(Self::Dilithium3),
            _ => None,
        }
    }
}
/// Desired behavior when negotiating constant-rate transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstantRateMode {
    BestEffort,
    Strict,
}
impl ConstantRateMode {
    /// Decode a mode from the constant-rate flags byte.
    pub const fn from_flags(flags: u8) -> Self {
        if flags & CONSTANT_RATE_FLAG_STRICT != 0 {
            Self::Strict
        } else {
            Self::BestEffort
        }
    }
    /// Encode the mode into the flags byte.
    pub const fn flags(self) -> u8 {
        match self {
            Self::BestEffort => 0,
            Self::Strict => CONSTANT_RATE_FLAG_STRICT,
        }
    }
}
impl ConstantRateCapability {
    /// Serialize the capability into the TLV payload used in the handshake.
    pub fn encode_value(&self) -> [u8; 4] {
        let cell_bytes = self.cell_bytes.to_le_bytes();
        [
            self.version,
            self.mode.flags(),
            cell_bytes[0],
            cell_bytes[1],
        ]
    }
}
/// `constant-rate-v1` capability parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstantRateCapability {
    /// Protocol version for the constant-rate capability.
    pub version: u8,
    /// Whether strict constant-rate transport is required.
    pub mode: ConstantRateMode,
    /// Cell size advertised in the capability.
    pub cell_bytes: u16,
}
/// GREASE TLV entry preserved during the handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GreaseEntry {
    /// GREASE TLV type.
    pub ty: u16,
    /// Raw GREASE payload bytes.
    pub value: Vec<u8>,
}
/// KEM advertisement (id + required bit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KemAdvertisement {
    /// Advertised KEM identifier.
    pub id: KemId,
    /// Whether the KEM is required for the session.
    pub required: bool,
}
/// Signature advertisement (id + required bit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignatureAdvertisement {
    /// Advertised signature identifier.
    pub id: SignatureId,
    /// Whether the signature is required for the session.
    pub required: bool,
}
/// Parsed capability vector supplied by a client.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClientAdvertisement {
    /// KEMs advertised by the client in preference order.
    pub kem: Vec<KemAdvertisement>,
    /// Signatures advertised by the client in preference order.
    pub signatures: Vec<SignatureAdvertisement>,
    /// Optional padding cell size requested by the client.
    pub padding: Option<u16>,
    /// Optional constant-rate capability requested by the client.
    pub constant_rate: Option<ConstantRateCapability>,
    /// Optional transcript commit expected by the client.
    pub transcript_commit: Option<[u8; 32]>,
    /// GREASE TLVs in the reserved `0x7Fxx` range preserved during parsing.
    pub grease: Vec<GreaseEntry>,
}
/// Relay-side capabilities advertised in the configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerCapabilities {
    /// KEMs supported by the relay.
    pub kem: Vec<KemAdvertisement>,
    /// Signatures supported by the relay.
    pub signatures: Vec<SignatureAdvertisement>,
    /// Padding cell size enforced by the relay.
    pub padding: u16,
    /// Optional transcript commit the relay expects to see.
    pub descriptor_commit: Option<[u8; 32]>,
    /// Role bits advertised by the relay (entry/middle/exit).
    pub role_bits: u8,
    /// Optional constant-rate capability supported by the relay.
    pub constant_rate: Option<ConstantRateCapability>,
}
impl ServerCapabilities {
    pub fn new(
        kem: Vec<KemAdvertisement>,
        signatures: Vec<SignatureAdvertisement>,
        padding: u16,
        descriptor_commit: Option<[u8; 32]>,
        role_bits: u8,
        constant_rate: Option<ConstantRateCapability>,
    ) -> Self {
        let safe_padding = PaddingConfig::clamp_cell_size(padding);
        Self {
            kem,
            signatures,
            padding: safe_padding,
            descriptor_commit,
            role_bits,
            constant_rate,
        }
    }
}
/// Negotiated capability selection echoed back to the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiatedCapabilities {
    /// KEM chosen for the session.
    pub kem: KemAdvertisement,
    /// Signatures chosen for the session.
    pub signatures: Vec<SignatureAdvertisement>,
    /// Padding cell size accepted for the session.
    pub padding: u16,
    /// Transcript commit echoed back to the client (if provided).
    pub descriptor_commit: Option<[u8; 32]>,
    /// GREASE TLVs preserved from the client advertisement.
    pub grease: Vec<GreaseEntry>,
    /// Constant-rate capability agreed for the session.
    pub constant_rate: Option<ConstantRateCapability>,
}
/// Errors surfaced while parsing or negotiating capability vectors.
#[derive(Debug, Error)]
pub enum CapabilityError {
    #[error("capability vector exceeded {MAX_CAP_VECTOR_LEN} bytes")]
    CapabilityVectorTooLarge,
    #[error("capability TLV truncated")]
    Truncated,
    #[error("capability TLV length {length} exceeds buffer")]
    LengthExceeds { length: usize },
    #[error("snnet.pqkem entry had invalid identifier {0:#04x}")]
    InvalidKemId(u8),
    #[error("snnet.pqsig entry had invalid identifier {0:#04x}")]
    InvalidSignatureId(u8),
    #[error("snnet.transcript_commit length must be 32 bytes")]
    InvalidTranscriptCommitLen,
    #[error("snnet.padding length must be 2 bytes")]
    InvalidPaddingLen,
    #[error("snnet.role length must be 1 byte")]
    InvalidRoleLen,
    #[error("snnet.role uses invalid role bits {0:#04x}")]
    InvalidRoleBits(u8),
    #[error("snnet.suite_list must contain at least one suite identifier")]
    InvalidSuiteList,
    #[error("capability TLV {ty:#06x} uses undefined flag bits {flags:#04x}")]
    InvalidCapabilityFlags { ty: u16, flags: u8 },
    #[error("duplicate singleton capability TLV {ty:#06x}")]
    DuplicateSingleton { ty: u16 },
    #[error("duplicate algorithm identifier {id:#04x} in capability TLV {ty:#06x}")]
    DuplicateAlgorithm { ty: u16, id: u8 },
    #[error("unknown non-GREASE capability TLV {ty:#06x}")]
    UnknownCapability { ty: u16 },
    #[error("capability TLV type order is non-canonical: {current:#06x} follows {previous:#06x}")]
    NonCanonicalTypeOrder { previous: u16, current: u16 },
    #[error("snnet.pqkem required capability {0:?} not supported by relay")]
    RequiredKemMissing(KemId),
    #[error("snnet.pqsig required capability {0:?} not supported by relay")]
    RequiredSignatureMissing(SignatureId),
    #[error("relay-required snnet.pqkem capability {0:?} not advertised by client")]
    RequiredKemMissingFromClient(KemId),
    #[error("relay-required snnet.pqsig capability {0:?} not advertised by client")]
    RequiredSignatureMissingFromClient(SignatureId),
    #[error("no mutually supported snnet.pqkem value")]
    NoMutualKem,
    #[error("no mutually supported snnet.pqsig value")]
    NoMutualSignature,
    #[error("client requested padding cell size {requested} but relay supports {supported}")]
    PaddingMismatch { requested: u16, supported: u16 },
    #[error("snnet.constant_rate length must be exactly 4 bytes")]
    ConstantRateInvalidLen,
    #[error("snnet.constant_rate requested unsupported version {0}")]
    ConstantRateUnsupportedVersion(u8),
    #[error("snnet.constant_rate requested but relay does not advertise support")]
    ConstantRateUnsupported,
    #[error("snnet.constant_rate strict mode requested but relay only supports best-effort")]
    ConstantRateStrictRequired,
    #[error(
        "snnet.constant_rate cell size {advertised} bytes is unsupported (expected {expected})"
    )]
    ConstantRateUnsupportedCellSize { advertised: u16, expected: u16 },
    #[error("snnet.transcript_commit mismatch")]
    TranscriptCommitMismatch,
    #[error("missing snnet.transcript_commit in client advertisement")]
    TranscriptCommitMissing,
    #[error("capability TLV {ty:#06x} value length {length} exceeds u16::MAX")]
    CapabilityValueTooLarge { ty: u16, length: usize },
}
/// Parse a capability vector into structured fields.
pub fn parse_client_advertisement(bytes: &[u8]) -> Result<ClientAdvertisement, CapabilityError> {
    if bytes.len() > MAX_CAP_VECTOR_LEN {
        return Err(CapabilityError::CapabilityVectorTooLarge);
    }
    let mut cursor = 0usize;
    let mut advert = ClientAdvertisement::default();
    let mut seen_singletons = 0u8;
    let mut previous_ty = None;
    while cursor + 4 <= bytes.len() {
        let ty = u16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]);
        if let Some(previous) = previous_ty
            && ty < previous
        {
            return Err(CapabilityError::NonCanonicalTypeOrder {
                previous,
                current: ty,
            });
        }
        previous_ty = Some(ty);
        let len = u16::from_be_bytes([bytes[cursor + 2], bytes[cursor + 3]]) as usize;
        cursor += 4;
        if cursor + len > bytes.len() {
            return Err(CapabilityError::LengthExceeds { length: len });
        }
        let value = &bytes[cursor..cursor + len];
        cursor += len;
        match ty {
            TYPE_PQ_KEM => {
                if value.len() != 2 {
                    return Err(CapabilityError::InvalidKemId(
                        value.first().copied().unwrap_or_default(),
                    ));
                }
                let Some(id) = KemId::from_code(value[0]) else {
                    return Err(CapabilityError::InvalidKemId(value[0]));
                };
                validate_capability_flags(ty, value[1], REQUIRED_FLAG)?;
                let required = (value[1] & REQUIRED_FLAG) != 0;
                if advert.kem.iter().any(|entry| entry.id == id) {
                    return Err(CapabilityError::DuplicateAlgorithm { ty, id: value[0] });
                }
                advert.kem.push(KemAdvertisement { id, required });
            }
            TYPE_PQ_SIG => {
                if value.len() != 2 {
                    return Err(CapabilityError::InvalidSignatureId(
                        value.first().copied().unwrap_or_default(),
                    ));
                }
                let Some(id) = SignatureId::from_code(value[0]) else {
                    return Err(CapabilityError::InvalidSignatureId(value[0]));
                };
                validate_capability_flags(ty, value[1], REQUIRED_FLAG)?;
                let required = (value[1] & REQUIRED_FLAG) != 0;
                if advert.signatures.iter().any(|entry| entry.id == id) {
                    return Err(CapabilityError::DuplicateAlgorithm { ty, id: value[0] });
                }
                advert
                    .signatures
                    .push(SignatureAdvertisement { id, required });
            }
            TYPE_TRANSCRIPT_COMMIT => {
                mark_singleton(&mut seen_singletons, SINGLETON_TRANSCRIPT_COMMIT, ty)?;
                if value.len() != 32 {
                    return Err(CapabilityError::InvalidTranscriptCommitLen);
                }
                let mut commit = [0u8; 32];
                commit.copy_from_slice(value);
                advert.transcript_commit = Some(commit);
            }
            TYPE_SUITE_LIST => {
                mark_singleton(&mut seen_singletons, SINGLETON_SUITE_LIST, ty)?;
                if value.is_empty() {
                    return Err(CapabilityError::InvalidSuiteList);
                }
            }
            TYPE_PADDING => {
                mark_singleton(&mut seen_singletons, SINGLETON_PADDING, ty)?;
                if value.len() != 2 {
                    return Err(CapabilityError::InvalidPaddingLen);
                }
                advert.padding = Some(u16::from_le_bytes([value[0], value[1]]));
            }
            TYPE_CONSTANT_RATE => {
                mark_singleton(&mut seen_singletons, SINGLETON_CONSTANT_RATE, ty)?;
                let capability = parse_constant_rate_capability(value)?;
                advert.constant_rate = Some(capability);
            }
            TYPE_ROLE => {
                mark_singleton(&mut seen_singletons, SINGLETON_ROLE, ty)?;
                if value.len() != 1 {
                    return Err(CapabilityError::InvalidRoleLen);
                }
                if value[0] == 0 || value[0] & !0x07 != 0 {
                    return Err(CapabilityError::InvalidRoleBits(value[0]));
                }
                // Clients normally omit `snnet.role`; after strict validation,
                // relay-side role selection remains authoritative.
            }
            ty if (0x7F00..=0x7FFF).contains(&ty) => {
                advert.grease.push(GreaseEntry {
                    ty,
                    value: value.to_vec(),
                });
            }
            _ => return Err(CapabilityError::UnknownCapability { ty }),
        }
    }
    if cursor != bytes.len() {
        return Err(CapabilityError::Truncated);
    }
    Ok(advert)
}
fn mark_singleton(seen: &mut u8, bit: u8, ty: u16) -> Result<(), CapabilityError> {
    if *seen & bit != 0 {
        return Err(CapabilityError::DuplicateSingleton { ty });
    }
    *seen |= bit;
    Ok(())
}
fn validate_capability_flags(ty: u16, flags: u8, allowed: u8) -> Result<(), CapabilityError> {
    if flags & !allowed != 0 {
        return Err(CapabilityError::InvalidCapabilityFlags { ty, flags });
    }
    Ok(())
}
fn parse_constant_rate_capability(value: &[u8]) -> Result<ConstantRateCapability, CapabilityError> {
    if value.len() != 4 {
        return Err(CapabilityError::ConstantRateInvalidLen);
    }
    let version = value[0];
    if version != 1 {
        return Err(CapabilityError::ConstantRateUnsupportedVersion(version));
    }
    validate_capability_flags(TYPE_CONSTANT_RATE, value[1], CONSTANT_RATE_FLAG_STRICT)?;
    let mode = ConstantRateMode::from_flags(value[1]);
    let cell_bytes = u16::from_le_bytes([value[2], value[3]]);
    let expected = CONSTANT_RATE_CELL_BYTES as u16;
    if cell_bytes != expected {
        return Err(CapabilityError::ConstantRateUnsupportedCellSize {
            advertised: cell_bytes,
            expected,
        });
    }
    Ok(ConstantRateCapability {
        version,
        mode,
        cell_bytes,
    })
}
/// Attempt to parse a 32-byte descriptor commit from a hex string.
pub fn parse_descriptor_commit_hex(hex_str: &str) -> Result<[u8; 32], CapabilityError> {
    let bytes =
        <[u8; 32]>::from_hex(hex_str).map_err(|_| CapabilityError::InvalidTranscriptCommitLen)?;
    Ok(bytes)
}
/// Negotiate the handshake capabilities with a client.
pub fn negotiate_capabilities(
    client: &ClientAdvertisement,
    server: &ServerCapabilities,
) -> Result<NegotiatedCapabilities, CapabilityError> {
    if let Some(expected) = server.descriptor_commit {
        match client.transcript_commit {
            Some(commit) if commit == expected => {}
            Some(_) => return Err(CapabilityError::TranscriptCommitMismatch),
            None => return Err(CapabilityError::TranscriptCommitMissing),
        }
    }
    for required in client.kem.iter().filter(|entry| entry.required) {
        if !server
            .kem
            .iter()
            .any(|server_entry| server_entry.id == required.id)
        {
            return Err(CapabilityError::RequiredKemMissing(required.id));
        }
    }
    for required in server.kem.iter().filter(|entry| entry.required) {
        if !client
            .kem
            .iter()
            .any(|client_entry| client_entry.id == required.id)
        {
            return Err(CapabilityError::RequiredKemMissingFromClient(required.id));
        }
    }
    let mut selected_kem = None;
    for server_pref in &server.kem {
        if let Some(client_entry) = client.kem.iter().find(|entry| entry.id == server_pref.id) {
            selected_kem = Some(KemAdvertisement {
                id: server_pref.id,
                required: server_pref.required || client_entry.required,
            });
            break;
        }
    }
    let Some(kem) = selected_kem else {
        return Err(CapabilityError::NoMutualKem);
    };
    for required in client.signatures.iter().filter(|entry| entry.required) {
        if !server
            .signatures
            .iter()
            .any(|server_entry| server_entry.id == required.id)
        {
            return Err(CapabilityError::RequiredSignatureMissing(required.id));
        }
    }
    for required in server.signatures.iter().filter(|entry| entry.required) {
        if !client
            .signatures
            .iter()
            .any(|client_entry| client_entry.id == required.id)
        {
            return Err(CapabilityError::RequiredSignatureMissingFromClient(
                required.id,
            ));
        }
    }
    let mut selected_sigs = Vec::new();
    for server_pref in &server.signatures {
        if let Some(client_entry) = client
            .signatures
            .iter()
            .find(|entry| entry.id == server_pref.id)
        {
            selected_sigs.push(SignatureAdvertisement {
                id: server_pref.id,
                required: server_pref.required || client_entry.required,
            });
        }
    }
    if selected_sigs.is_empty() {
        return Err(CapabilityError::NoMutualSignature);
    }
    if let Some(requested_padding) = client.padding
        && requested_padding != server.padding
    {
        return Err(CapabilityError::PaddingMismatch {
            requested: requested_padding,
            supported: server.padding,
        });
    }
    let negotiated_constant_rate = match (client.constant_rate, server.constant_rate) {
        (Some(request), Some(supported)) => {
            if request.version != supported.version {
                return Err(CapabilityError::ConstantRateUnsupportedVersion(
                    request.version,
                ));
            }
            if request.cell_bytes != supported.cell_bytes {
                return Err(CapabilityError::ConstantRateUnsupportedCellSize {
                    advertised: request.cell_bytes,
                    expected: supported.cell_bytes,
                });
            }
            if matches!(request.mode, ConstantRateMode::Strict)
                && matches!(supported.mode, ConstantRateMode::BestEffort)
            {
                return Err(CapabilityError::ConstantRateStrictRequired);
            }
            Some(ConstantRateCapability {
                version: supported.version,
                mode: if matches!(request.mode, ConstantRateMode::Strict)
                    || matches!(supported.mode, ConstantRateMode::Strict)
                {
                    ConstantRateMode::Strict
                } else {
                    ConstantRateMode::BestEffort
                },
                cell_bytes: supported.cell_bytes,
            })
        }
        (Some(request), None) => {
            if matches!(request.mode, ConstantRateMode::Strict) {
                return Err(CapabilityError::ConstantRateUnsupported);
            }
            None
        }
        (None, Some(supported)) => Some(supported),
        (None, None) => None,
    };
    Ok(NegotiatedCapabilities {
        kem,
        signatures: selected_sigs,
        padding: server.padding,
        descriptor_commit: server.descriptor_commit,
        grease: client.grease.clone(),
        constant_rate: negotiated_constant_rate,
    })
}
/// Encode the relay response capability vector reflecting the negotiated values.
pub fn encode_relay_advertisement(
    negotiated: &NegotiatedCapabilities,
    role_bits: u8,
) -> Result<Vec<u8>, CapabilityError> {
    let mut out = Vec::new();
    push_tlv(
        &mut out,
        TYPE_PQ_KEM,
        &[negotiated.kem.id.code(), flag_byte(negotiated.kem.required)],
    )?;
    for sig in &negotiated.signatures {
        push_tlv(
            &mut out,
            TYPE_PQ_SIG,
            &[sig.id.code(), flag_byte(sig.required)],
        )?;
    }
    if let Some(commit) = negotiated.descriptor_commit {
        push_tlv(&mut out, TYPE_TRANSCRIPT_COMMIT, &commit)?;
    }
    push_tlv(&mut out, TYPE_ROLE, &[role_bits])?;
    push_tlv(&mut out, TYPE_PADDING, &negotiated.padding.to_le_bytes())?;
    if let Some(constant_rate) = negotiated.constant_rate {
        let value = constant_rate.encode_value();
        push_tlv(&mut out, TYPE_CONSTANT_RATE, &value)?;
    }
    for grease in &negotiated.grease {
        push_tlv(&mut out, grease.ty, &grease.value)?;
    }
    Ok(out)
}
fn flag_byte(required: bool) -> u8 {
    if required { REQUIRED_FLAG } else { 0 }
}
fn push_tlv(buffer: &mut Vec<u8>, ty: u16, value: &[u8]) -> Result<(), CapabilityError> {
    let len = u16::try_from(value.len()).map_err(|_| CapabilityError::CapabilityValueTooLarge {
        ty,
        length: value.len(),
    })?;
    let encoded_len = buffer
        .len()
        .checked_add(4)
        .and_then(|encoded_len| encoded_len.checked_add(value.len()))
        .ok_or(CapabilityError::CapabilityVectorTooLarge)?;
    if encoded_len > MAX_CAP_VECTOR_LEN {
        return Err(CapabilityError::CapabilityVectorTooLarge);
    }
    buffer.extend_from_slice(&ty.to_be_bytes());
    buffer.extend_from_slice(&len.to_be_bytes());
    buffer.extend_from_slice(value);
    Ok(())
}
impl fmt::Display for KemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KemId::MlKem512 => write!(f, "ml-kem-512"),
            KemId::MlKem768 => write!(f, "ml-kem-768"),
            KemId::MlKem1024 => write!(f, "ml-kem-1024"),
        }
    }
}
impl fmt::Display for SignatureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignatureId::Dilithium3 => write!(f, "dilithium3"),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample_constant_rate(mode: ConstantRateMode) -> ConstantRateCapability {
        ConstantRateCapability {
            version: 1,
            mode,
            cell_bytes: CONSTANT_RATE_CELL_BYTES as u16,
        }
    }
    fn sample_client_vector() -> Vec<u8> {
        let mut bytes = Vec::new();
        push_tlv(
            &mut bytes,
            TYPE_PQ_KEM,
            &[KemId::MlKem768.code(), REQUIRED_FLAG],
        )
        .expect("pqkem TLV");
        push_tlv(
            &mut bytes,
            TYPE_PQ_SIG,
            &[SignatureId::Dilithium3.code(), REQUIRED_FLAG],
        )
        .expect("pqsig TLV");
        push_tlv(&mut bytes, TYPE_TRANSCRIPT_COMMIT, &[0xAA; 32]).expect("commit TLV");
        push_tlv(&mut bytes, TYPE_SUITE_LIST, &[0x84, 0x05]).expect("suite-list TLV");
        push_tlv(&mut bytes, TYPE_PADDING, &1024u16.to_le_bytes()).expect("padding TLV");
        let value = sample_constant_rate(ConstantRateMode::BestEffort).encode_value();
        push_tlv(&mut bytes, TYPE_CONSTANT_RATE, &value).expect("constant-rate TLV");
        push_tlv(&mut bytes, 0x7F10, &[0xDE, 0xAD, 0xBE, 0xEF]).expect("GREASE TLV");
        bytes
    }
    fn sample_server_caps() -> ServerCapabilities {
        ServerCapabilities::new(
            vec![KemAdvertisement {
                id: KemId::MlKem768,
                required: true,
            }],
            vec![SignatureAdvertisement {
                id: SignatureId::Dilithium3,
                required: true,
            }],
            1024,
            Some([0xAA; 32]),
            0x01,
            Some(sample_constant_rate(ConstantRateMode::Strict)),
        )
    }
    #[test]
    fn parse_and_negotiate_capabilities() {
        let bytes = sample_client_vector();
        let client = parse_client_advertisement(&bytes).expect("parse client vector");
        assert_eq!(client.kem.len(), 1);
        assert_eq!(client.signatures.len(), 1);
        assert_eq!(client.padding, Some(1024));
        assert_eq!(
            client.constant_rate,
            Some(sample_constant_rate(ConstantRateMode::BestEffort))
        );
        assert_eq!(client.grease.len(), 1);
        assert_eq!(client.transcript_commit, Some([0xAA; 32]));
        let negotiated =
            negotiate_capabilities(&client, &sample_server_caps()).expect("negotiate capabilities");
        assert_eq!(negotiated.kem.id, KemId::MlKem768);
        assert!(negotiated.kem.required);
        assert_eq!(negotiated.signatures[0].id, SignatureId::Dilithium3);
        assert_eq!(negotiated.padding, 1024);
        assert_eq!(negotiated.grease.len(), 1);
        assert_eq!(
            negotiated.constant_rate,
            Some(sample_constant_rate(ConstantRateMode::Strict))
        );
        let relay_bytes = encode_relay_advertisement(&negotiated, 0x01).expect("relay caps");
        assert!(!relay_bytes.is_empty());
        assert!(
            relay_bytes
                .windows(6)
                .any(|window| window == [0x02, 0x02, 0x00, 0x02, 0x00, 0x04])
        );
    }
    #[test]
    fn parser_rejects_duplicate_singletons_and_preserves_distinct_algorithms() {
        let singleton_values = vec![
            (TYPE_TRANSCRIPT_COMMIT, vec![0xAA; 32]),
            (TYPE_SUITE_LIST, vec![0x84, 0x05]),
            (TYPE_ROLE, vec![0x01]),
            (TYPE_PADDING, 1024u16.to_le_bytes().to_vec()),
            (
                TYPE_CONSTANT_RATE,
                sample_constant_rate(ConstantRateMode::Strict)
                    .encode_value()
                    .to_vec(),
            ),
        ];
        for (ty, value) in singleton_values {
            let mut bytes = Vec::new();
            push_tlv(&mut bytes, ty, &value).expect("first singleton");
            push_tlv(&mut bytes, ty, &value).expect("duplicate singleton");
            assert!(matches!(
                parse_client_advertisement(&bytes),
                Err(CapabilityError::DuplicateSingleton { ty: duplicate }) if duplicate == ty
            ));
        }

        let mut multi = Vec::new();
        push_tlv(&mut multi, TYPE_PQ_KEM, &[KemId::MlKem512.code(), 0]).expect("first KEM");
        push_tlv(
            &mut multi,
            TYPE_PQ_KEM,
            &[KemId::MlKem768.code(), REQUIRED_FLAG],
        )
        .expect("second KEM");
        push_tlv(
            &mut multi,
            TYPE_PQ_SIG,
            &[SignatureId::Dilithium3.code(), 0],
        )
        .expect("first signature");
        push_tlv(&mut multi, 0x7F10, &[1]).expect("first GREASE");
        push_tlv(&mut multi, 0x7F10, &[2]).expect("second GREASE");
        let parsed = parse_client_advertisement(&multi).expect("multi-entry types stay legal");
        assert_eq!(parsed.kem.len(), 2);
        assert_eq!(parsed.signatures.len(), 1);
        assert_eq!(parsed.grease.len(), 2);
        assert_eq!(parsed.grease[0].value, [1]);
        assert_eq!(parsed.grease[1].value, [2]);
    }
    #[test]
    fn parser_rejects_duplicate_algorithm_ids_even_when_flags_differ() {
        for (ty, id) in [
            (TYPE_PQ_KEM, KemId::MlKem768.code()),
            (TYPE_PQ_SIG, SignatureId::Dilithium3.code()),
        ] {
            let mut bytes = Vec::new();
            push_tlv(&mut bytes, ty, &[id, 0]).expect("first algorithm");
            push_tlv(&mut bytes, ty, &[id, REQUIRED_FLAG]).expect("duplicate algorithm");
            assert!(matches!(
                parse_client_advertisement(&bytes),
                Err(CapabilityError::DuplicateAlgorithm {
                    ty: duplicate_ty,
                    id: duplicate_id,
                }) if duplicate_ty == ty && duplicate_id == id
            ));
        }
    }
    #[test]
    fn parser_rejects_unknown_types_malformed_role_and_reserved_flags() {
        let mut unknown = Vec::new();
        push_tlv(&mut unknown, 0x1234, &[1]).expect("unknown TLV");
        assert!(matches!(
            parse_client_advertisement(&unknown),
            Err(CapabilityError::UnknownCapability { ty: 0x1234 })
        ));

        for role in [Vec::new(), vec![1, 2]] {
            let mut bytes = Vec::new();
            push_tlv(&mut bytes, TYPE_ROLE, &role).expect("role TLV");
            assert!(matches!(
                parse_client_advertisement(&bytes),
                Err(CapabilityError::InvalidRoleLen)
            ));
        }
        for role in [0x00, 0x08, 0x80, 0xFF] {
            let mut bytes = Vec::new();
            push_tlv(&mut bytes, TYPE_ROLE, &[role]).expect("role TLV");
            assert!(matches!(
                parse_client_advertisement(&bytes),
                Err(CapabilityError::InvalidRoleBits(rejected)) if rejected == role
            ));
        }

        for (ty, id) in [
            (TYPE_PQ_KEM, KemId::MlKem768.code()),
            (TYPE_PQ_SIG, SignatureId::Dilithium3.code()),
        ] {
            let mut bytes = Vec::new();
            push_tlv(&mut bytes, ty, &[id, 0x80]).expect("flagged TLV");
            assert!(matches!(
                parse_client_advertisement(&bytes),
                Err(CapabilityError::InvalidCapabilityFlags {
                    ty: rejected,
                    flags: 0x80,
                }) if rejected == ty
            ));
        }

        let mut constant_rate = sample_constant_rate(ConstantRateMode::BestEffort).encode_value();
        constant_rate[1] = 0x80;
        let mut bytes = Vec::new();
        push_tlv(&mut bytes, TYPE_CONSTANT_RATE, &constant_rate).expect("constant-rate TLV");
        assert!(matches!(
            parse_client_advertisement(&bytes),
            Err(CapabilityError::InvalidCapabilityFlags {
                ty: TYPE_CONSTANT_RATE,
                flags: 0x80,
            })
        ));

        let mut empty_suite_list = Vec::new();
        push_tlv(&mut empty_suite_list, TYPE_SUITE_LIST, &[]).expect("suite-list TLV");
        assert!(matches!(
            parse_client_advertisement(&empty_suite_list),
            Err(CapabilityError::InvalidSuiteList)
        ));
    }
    #[test]
    fn parser_rejects_decreasing_tlv_type_order() {
        let mut bytes = Vec::new();
        push_tlv(&mut bytes, TYPE_PADDING, &1024_u16.to_le_bytes()).expect("padding TLV");
        push_tlv(
            &mut bytes,
            TYPE_PQ_KEM,
            &[KemId::MlKem768.code(), REQUIRED_FLAG],
        )
        .expect("out-of-order KEM TLV");
        assert!(matches!(
            parse_client_advertisement(&bytes),
            Err(CapabilityError::NonCanonicalTypeOrder {
                previous: TYPE_PADDING,
                current: TYPE_PQ_KEM,
            })
        ));
    }
    #[test]
    fn required_kem_missing_fails() {
        let mut bytes = Vec::new();
        push_tlv(
            &mut bytes,
            TYPE_PQ_KEM,
            &[KemId::MlKem1024.code(), REQUIRED_FLAG],
        )
        .expect("pqkem TLV");
        push_tlv(
            &mut bytes,
            TYPE_PQ_SIG,
            &[SignatureId::Dilithium3.code(), REQUIRED_FLAG],
        )
        .expect("pqsig TLV");
        push_tlv(&mut bytes, TYPE_TRANSCRIPT_COMMIT, &[0xAA; 32]).expect("commit TLV");
        let client = parse_client_advertisement(&bytes).expect("parse client");
        let err = negotiate_capabilities(&client, &sample_server_caps()).unwrap_err();
        assert!(matches!(
            err,
            CapabilityError::RequiredKemMissing(KemId::MlKem1024)
        ));
    }
    #[test]
    fn relay_required_kem_must_be_advertised_by_client() {
        let mut client = parse_client_advertisement(&sample_client_vector()).expect("parse");
        client.kem = vec![KemAdvertisement {
            id: KemId::MlKem512,
            required: false,
        }];
        let server = ServerCapabilities::new(
            vec![
                KemAdvertisement {
                    id: KemId::MlKem1024,
                    required: true,
                },
                KemAdvertisement {
                    id: KemId::MlKem512,
                    required: false,
                },
            ],
            sample_server_caps().signatures,
            1024,
            Some([0xAA; 32]),
            0x01,
            None,
        );
        let err = negotiate_capabilities(&client, &server).unwrap_err();
        assert!(matches!(
            err,
            CapabilityError::RequiredKemMissingFromClient(KemId::MlKem1024)
        ));
    }
    #[test]
    fn relay_required_signature_must_be_advertised_by_client() {
        let mut client = parse_client_advertisement(&sample_client_vector()).expect("parse");
        client.signatures.clear();
        let err = negotiate_capabilities(&client, &sample_server_caps()).unwrap_err();
        assert!(matches!(
            err,
            CapabilityError::RequiredSignatureMissingFromClient(SignatureId::Dilithium3)
        ));
    }
    #[test]
    fn relay_required_algorithms_remain_required_in_response() {
        let mut client = parse_client_advertisement(&sample_client_vector()).expect("parse");
        client.kem[0].required = false;
        client.signatures[0].required = false;

        let negotiated = negotiate_capabilities(&client, &sample_server_caps()).expect("negotiate");
        assert!(negotiated.kem.required);
        assert!(negotiated.signatures[0].required);

        let encoded = encode_relay_advertisement(&negotiated, 0x01).expect("relay caps");
        assert!(encoded.windows(6).any(|window| {
            window
                == [
                    (TYPE_PQ_KEM >> 8) as u8,
                    TYPE_PQ_KEM as u8,
                    0,
                    2,
                    KemId::MlKem768.code(),
                    REQUIRED_FLAG,
                ]
        }));
        assert!(encoded.windows(6).any(|window| {
            window
                == [
                    (TYPE_PQ_SIG >> 8) as u8,
                    TYPE_PQ_SIG as u8,
                    0,
                    2,
                    SignatureId::Dilithium3.code(),
                    REQUIRED_FLAG,
                ]
        }));
    }
    #[test]
    fn transcript_commit_mismatch_detected() {
        let mut client = parse_client_advertisement(&sample_client_vector()).expect("parse");
        client.transcript_commit = Some([0xBB; 32]);
        let err = negotiate_capabilities(&client, &sample_server_caps()).unwrap_err();
        assert!(matches!(err, CapabilityError::TranscriptCommitMismatch));
    }
    #[test]
    fn server_capabilities_clamp_padding_to_mtu_limit() {
        let max = PaddingConfig::max_cell_size_bytes();
        let caps =
            ServerCapabilities::new(vec![], vec![], max.saturating_add(42), None, 0x01, None);
        assert_eq!(caps.padding, max);
    }
    #[test]
    fn constant_rate_strict_requires_server_support() {
        let mut client = parse_client_advertisement(&sample_client_vector()).expect("parse");
        client.constant_rate = Some(sample_constant_rate(ConstantRateMode::Strict));
        let server = ServerCapabilities::new(
            vec![KemAdvertisement {
                id: KemId::MlKem768,
                required: true,
            }],
            vec![SignatureAdvertisement {
                id: SignatureId::Dilithium3,
                required: true,
            }],
            1024,
            Some([0xAA; 32]),
            0x01,
            Some(sample_constant_rate(ConstantRateMode::BestEffort)),
        );
        let err = negotiate_capabilities(&client, &server).unwrap_err();
        assert!(matches!(err, CapabilityError::ConstantRateStrictRequired));
    }
    #[test]
    fn constant_rate_version_mismatch_rejected() {
        let mut bytes = Vec::new();
        let mut upgraded = sample_constant_rate(ConstantRateMode::BestEffort).encode_value();
        upgraded[0] = 2;
        push_tlv(&mut bytes, TYPE_CONSTANT_RATE, &upgraded).expect("constant-rate TLV");
        assert!(matches!(
            parse_client_advertisement(&bytes),
            Err(CapabilityError::ConstantRateUnsupportedVersion(2))
        ));
    }
    #[test]
    fn constant_rate_payload_is_exactly_four_bytes() {
        for payload in [vec![1, 0, 0], vec![1, 0, 0, 4, 0]] {
            let mut bytes = Vec::new();
            push_tlv(&mut bytes, TYPE_CONSTANT_RATE, &payload).expect("constant-rate TLV");
            assert!(matches!(
                parse_client_advertisement(&bytes),
                Err(CapabilityError::ConstantRateInvalidLen)
            ));
        }
    }
    #[test]
    fn constant_rate_best_effort_allows_degraded_session() {
        let client = parse_client_advertisement(&sample_client_vector()).expect("parse");
        let server = ServerCapabilities::new(
            vec![KemAdvertisement {
                id: KemId::MlKem768,
                required: true,
            }],
            vec![SignatureAdvertisement {
                id: SignatureId::Dilithium3,
                required: true,
            }],
            1024,
            Some([0xAA; 32]),
            0x01,
            None,
        );
        let negotiated = negotiate_capabilities(&client, &server).expect("negotiate");
        assert!(negotiated.constant_rate.is_none());
    }
    #[test]
    fn constant_rate_strict_request_rejected_when_server_disabled() {
        let mut client = parse_client_advertisement(&sample_client_vector()).expect("parse");
        client.constant_rate = Some(sample_constant_rate(ConstantRateMode::Strict));
        let server = ServerCapabilities::new(
            vec![KemAdvertisement {
                id: KemId::MlKem768,
                required: true,
            }],
            vec![SignatureAdvertisement {
                id: SignatureId::Dilithium3,
                required: true,
            }],
            1024,
            Some([0xAA; 32]),
            0x01,
            None,
        );
        let err = negotiate_capabilities(&client, &server).unwrap_err();
        assert!(matches!(err, CapabilityError::ConstantRateUnsupported));
    }
    #[test]
    fn constant_rate_server_enforces_profile_when_client_omits_tlv() {
        let mut client = parse_client_advertisement(&sample_client_vector()).expect("parse");
        client.constant_rate = None;
        let negotiated = negotiate_capabilities(&client, &sample_server_caps()).expect("negotiate");
        assert_eq!(
            negotiated.constant_rate,
            Some(sample_constant_rate(ConstantRateMode::Strict)),
            "server should enforce its configured constant-rate profile even if the viewer omits the TLV",
        );
        let relay_vector = encode_relay_advertisement(&negotiated, 0x01).expect("relay caps");
        assert!(
            relay_vector.windows(8).any(|window| {
                window
                    == [
                        (TYPE_CONSTANT_RATE >> 8) as u8,
                        (TYPE_CONSTANT_RATE & 0xFF) as u8,
                        0x00,
                        0x04,
                        1,
                        ConstantRateMode::Strict.flags(),
                        0x00,
                        0x04,
                    ]
            }),
            "relay advertisement must include the strict constant-rate TLV"
        );
    }
    #[test]
    fn encode_relay_advertisement_rejects_oversized_tlv_value_without_truncation() {
        let mut negotiated = negotiate_capabilities(
            &parse_client_advertisement(&sample_client_vector()).unwrap(),
            &sample_server_caps(),
        )
        .expect("negotiate");
        negotiated.grease.push(GreaseEntry {
            ty: 0x7F12,
            value: vec![0xAA; usize::from(u16::MAX) + 1],
        });
        let err = encode_relay_advertisement(&negotiated, 0x01)
            .expect_err("oversized TLV value must fail");
        assert!(matches!(
            err,
            CapabilityError::CapabilityValueTooLarge {
                ty: 0x7F12,
                length
            } if length == usize::from(u16::MAX) + 1
        ));
    }
    #[test]
    fn encode_relay_advertisement_enforces_aggregate_vector_limit() {
        let mut negotiated = negotiate_capabilities(
            &parse_client_advertisement(&sample_client_vector()).unwrap(),
            &sample_server_caps(),
        )
        .expect("negotiate");
        negotiated.grease.clear();
        let base = encode_relay_advertisement(&negotiated, 0x01).expect("base relay caps");
        let grease_value_len = MAX_CAP_VECTOR_LEN
            .checked_sub(base.len() + 4)
            .expect("base response leaves room for one GREASE header");
        negotiated.grease.push(GreaseEntry {
            ty: 0x7F12,
            value: vec![0xAA; grease_value_len],
        });
        let exact = encode_relay_advertisement(&negotiated, 0x01)
            .expect("exact-limit relay capability vector must encode");
        assert_eq!(exact.len(), MAX_CAP_VECTOR_LEN);

        negotiated.grease[0].value.push(0xAA);
        assert!(matches!(
            encode_relay_advertisement(&negotiated, 0x01),
            Err(CapabilityError::CapabilityVectorTooLarge)
        ));
    }
}
