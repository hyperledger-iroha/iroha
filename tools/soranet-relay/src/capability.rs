//! Capability negotiation helpers for the SoraNet relay handshake.
//!
//! This module implements the TLV parsing and negotiation rules documented in
//! `docs/source/soranet_handshake.md`. It is intentionally deterministic so
//! transcript hashes computed by clients and relays match byte-for-byte.

use std::fmt;

use hex::FromHex;
pub use iroha_crypto::soranet::handshake::CapabilityWarning;
use thiserror::Error;

use crate::{
    config::PaddingConfig,
    constant_rate::CONSTANT_RATE_CELL_BYTES,
    scheduler::{Cell, CellClass},
};

/// Maximum capability vector size we accept during negotiation.
pub const MAX_CAP_VECTOR_LEN: usize = 4096;

/// `snnet.pqkem` TLV type.
pub const TYPE_PQ_KEM: u16 = 0x0101;
/// `snnet.pqsig` TLV type.
pub const TYPE_PQ_SIG: u16 = 0x0102;
/// `snnet.transcript_commit` TLV type.
pub const TYPE_TRANSCRIPT_COMMIT: u16 = 0x0103;
/// `snnet.role` TLV type.
pub const TYPE_ROLE: u16 = 0x0201;
/// `snnet.padding` TLV type.
pub const TYPE_PADDING: u16 = 0x0202;
/// `snnet.constant_rate` TLV type.
pub const TYPE_CONSTANT_RATE: u16 = 0x0203;

const REQUIRED_FLAG: u8 = 0x01;
const CONSTANT_RATE_FLAG_STRICT: u8 = 0x01;

/// Recognised ML-KEM variants exchanged during capability negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KemId {
    /// Classical-only profile (no PQ KEM).
    Classic,
    /// Kyber768 (ML-KEM-768).
    MlKem768,
    /// Kyber1024 (ML-KEM-1024).
    MlKem1024,
}

impl KemId {
    /// Return the wire code associated with this KEM identifier.
    pub const fn code(self) -> u8 {
        match self {
            Self::Classic => 0x00,
            Self::MlKem768 => 0x01,
            Self::MlKem1024 => 0x02,
        }
    }

    /// Convert a wire code into a [`KemId`], rejecting unknown codes.
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            0x00 => Some(Self::Classic),
            0x01 => Some(Self::MlKem768),
            0x02 => Some(Self::MlKem1024),
            _ => None,
        }
    }
}

/// Recognised signature variants exchanged during capability negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureId {
    /// Ed25519 (classical baseline).
    Ed25519,
    /// Dilithium3 (preferred PQ signature).
    Dilithium3,
    /// Falcon-512 (optional PQ signature).
    Falcon512,
}

impl SignatureId {
    /// Return the wire code associated with this signature identifier.
    pub const fn code(self) -> u8 {
        match self {
            Self::Ed25519 => 0x00,
            Self::Dilithium3 => 0x01,
            Self::Falcon512 => 0x02,
        }
    }

    /// Convert a wire code into a [`SignatureId`], rejecting unknown codes.
    pub const fn from_code(code: u8) -> Option<Self> {
        match code {
            0x00 => Some(Self::Ed25519),
            0x01 => Some(Self::Dilithium3),
            0x02 => Some(Self::Falcon512),
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
    pub fn encode_value(&self) -> Vec<u8> {
        let mut value = Vec::with_capacity(4 + usize::from(self.cell_bytes));
        value.push(self.version);
        value.push(self.mode.flags());
        value.extend_from_slice(&self.cell_bytes.to_le_bytes());
        let sample = Cell::dummy().to_bytes();
        debug_assert_eq!(
            sample.len(),
            usize::from(self.cell_bytes),
            "constant-rate capability sample must match declared cell size"
        );
        value.extend_from_slice(&sample);
        value
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
    /// Vendor/unknown GREASE TLVs preserved during parsing.
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
    #[error("snnet.pqkem required capability {0:?} not supported by relay")]
    RequiredKemMissing(KemId),
    #[error("snnet.pqsig required capability {0:?} not supported by relay")]
    RequiredSignatureMissing(SignatureId),
    #[error("no mutually supported snnet.pqkem value")]
    NoMutualKem,
    #[error("no mutually supported snnet.pqsig value")]
    NoMutualSignature,
    #[error("client requested padding cell size {requested} but relay supports {supported}")]
    PaddingMismatch { requested: u16, supported: u16 },
    #[error("snnet.constant_rate length must be 2 bytes")]
    ConstantRateInvalidLen,
    #[error("snnet.constant_rate requested unsupported version {0}")]
    ConstantRateUnsupportedVersion(u8),
    #[error("snnet.constant_rate requested but relay does not advertise support")]
    ConstantRateUnsupported,
    #[error("snnet.constant_rate strict mode requested but relay only supports best-effort")]
    ConstantRateStrictRequired,
    #[error(
        "snnet.constant_rate envelope sample length {actual} does not match declared cell size {declared}"
    )]
    ConstantRateEnvelopeLength { declared: u16, actual: usize },
    #[error("snnet.constant_rate envelope sample is invalid")]
    ConstantRateEnvelopeInvalid,
    #[error(
        "snnet.constant_rate cell size {advertised} bytes is unsupported (expected {expected})"
    )]
    ConstantRateUnsupportedCellSize { advertised: u16, expected: u16 },
    #[error("snnet.transcript_commit mismatch")]
    TranscriptCommitMismatch,
    #[error("missing snnet.transcript_commit in client advertisement")]
    TranscriptCommitMissing,
}

/// Parse a capability vector into structured fields.
pub fn parse_client_advertisement(bytes: &[u8]) -> Result<ClientAdvertisement, CapabilityError> {
    if bytes.len() > MAX_CAP_VECTOR_LEN {
        return Err(CapabilityError::CapabilityVectorTooLarge);
    }
    let mut cursor = 0usize;
    let mut advert = ClientAdvertisement::default();

    while cursor + 4 <= bytes.len() {
        let ty = u16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]);
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
                let required = (value[1] & REQUIRED_FLAG) != 0;
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
                let required = (value[1] & REQUIRED_FLAG) != 0;
                advert
                    .signatures
                    .push(SignatureAdvertisement { id, required });
            }
            TYPE_TRANSCRIPT_COMMIT => {
                if value.len() != 32 {
                    return Err(CapabilityError::InvalidTranscriptCommitLen);
                }
                let mut commit = [0u8; 32];
                commit.copy_from_slice(value);
                advert.transcript_commit = Some(commit);
            }
            TYPE_PADDING => {
                if value.len() != 2 {
                    return Err(CapabilityError::InvalidPaddingLen);
                }
                advert.padding = Some(u16::from_le_bytes([value[0], value[1]]));
            }
            TYPE_CONSTANT_RATE => {
                let capability = parse_constant_rate_capability(value)?;
                advert.constant_rate = Some(capability);
            }
            TYPE_ROLE => {
                // Clients normally omit `snnet.role`, so we ignore it silently.
            }
            ty if (0x7F00..=0x7FFF).contains(&ty) => {
                advert.grease.push(GreaseEntry {
                    ty,
                    value: value.to_vec(),
                });
            }
            _ => {
                advert.grease.push(GreaseEntry {
                    ty,
                    value: value.to_vec(),
                });
            }
        }
    }

    if cursor != bytes.len() {
        return Err(CapabilityError::Truncated);
    }

    Ok(advert)
}

fn parse_constant_rate_capability(value: &[u8]) -> Result<ConstantRateCapability, CapabilityError> {
    if value.len() < 4 {
        return Err(CapabilityError::ConstantRateInvalidLen);
    }
    let version = value[0];
    let mode = ConstantRateMode::from_flags(value[1]);
    let cell_bytes = u16::from_le_bytes([value[2], value[3]]);
    let sample = &value[4..];
    if sample.len() != usize::from(cell_bytes) {
        return Err(CapabilityError::ConstantRateEnvelopeLength {
            declared: cell_bytes,
            actual: sample.len(),
        });
    }
    let expected = CONSTANT_RATE_CELL_BYTES as u16;
    if cell_bytes != expected {
        return Err(CapabilityError::ConstantRateUnsupportedCellSize {
            advertised: cell_bytes,
            expected,
        });
    }
    let Some(cell) = Cell::from_bytes(sample, CellClass::Bulk) else {
        return Err(CapabilityError::ConstantRateEnvelopeInvalid);
    };
    if !cell.is_dummy || !cell.data.is_empty() {
        return Err(CapabilityError::ConstantRateEnvelopeInvalid);
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

    let mut selected_kem = None;
    for server_pref in &server.kem {
        if let Some(client_entry) = client.kem.iter().find(|entry| entry.id == server_pref.id) {
            selected_kem = Some(KemAdvertisement {
                id: server_pref.id,
                required: client_entry.required,
            });
            break;
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

    let Some(kem) = selected_kem else {
        return Err(CapabilityError::NoMutualKem);
    };

    let mut selected_sigs = Vec::new();
    for server_pref in &server.signatures {
        if let Some(client_entry) = client
            .signatures
            .iter()
            .find(|entry| entry.id == server_pref.id)
        {
            selected_sigs.push(SignatureAdvertisement {
                id: server_pref.id,
                required: client_entry.required,
            });
        }
    }

    for required in client.signatures.iter().filter(|entry| entry.required) {
        if !server
            .signatures
            .iter()
            .any(|server_entry| server_entry.id == required.id)
        {
            return Err(CapabilityError::RequiredSignatureMissing(required.id));
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
pub fn encode_relay_advertisement(negotiated: &NegotiatedCapabilities, role_bits: u8) -> Vec<u8> {
    let mut out = Vec::new();
    push_tlv(
        &mut out,
        TYPE_PQ_KEM,
        &[negotiated.kem.id.code(), flag_byte(negotiated.kem.required)],
    );

    for sig in &negotiated.signatures {
        push_tlv(
            &mut out,
            TYPE_PQ_SIG,
            &[sig.id.code(), flag_byte(sig.required)],
        );
    }

    if let Some(commit) = negotiated.descriptor_commit {
        push_tlv(&mut out, TYPE_TRANSCRIPT_COMMIT, &commit);
    }

    push_tlv(&mut out, TYPE_ROLE, &[role_bits]);
    push_tlv(&mut out, TYPE_PADDING, &negotiated.padding.to_le_bytes());
    if let Some(constant_rate) = negotiated.constant_rate {
        let value = constant_rate.encode_value();
        push_tlv(&mut out, TYPE_CONSTANT_RATE, &value);
    }

    for grease in &negotiated.grease {
        push_tlv(&mut out, grease.ty, &grease.value);
    }

    out
}

fn flag_byte(required: bool) -> u8 {
    if required { REQUIRED_FLAG } else { 0 }
}

fn push_tlv(buffer: &mut Vec<u8>, ty: u16, value: &[u8]) {
    buffer.extend_from_slice(&ty.to_be_bytes());
    buffer.extend_from_slice(&(value.len() as u16).to_be_bytes());
    buffer.extend_from_slice(value);
}

impl fmt::Display for KemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KemId::Classic => write!(f, "classical"),
            KemId::MlKem768 => write!(f, "ml-kem-768"),
            KemId::MlKem1024 => write!(f, "ml-kem-1024"),
        }
    }
}

impl fmt::Display for SignatureId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignatureId::Ed25519 => write!(f, "ed25519"),
            SignatureId::Dilithium3 => write!(f, "dilithium3"),
            SignatureId::Falcon512 => write!(f, "falcon512"),
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
        );
        push_tlv(
            &mut bytes,
            TYPE_PQ_SIG,
            &[SignatureId::Dilithium3.code(), REQUIRED_FLAG],
        );
        push_tlv(&mut bytes, TYPE_PADDING, &1024u16.to_le_bytes());
        let value = sample_constant_rate(ConstantRateMode::BestEffort).encode_value();
        push_tlv(&mut bytes, TYPE_CONSTANT_RATE, &value);
        push_tlv(&mut bytes, TYPE_TRANSCRIPT_COMMIT, &[0xAA; 32]);
        push_tlv(&mut bytes, 0x7F10, &[0xDE, 0xAD, 0xBE, 0xEF]);
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

        let relay_bytes = encode_relay_advertisement(&negotiated, 0x01);
        assert!(!relay_bytes.is_empty());
        assert!(
            relay_bytes
                .windows(6)
                .any(|window| window == [0x02, 0x02, 0x00, 0x02, 0x00, 0x04])
        );
    }

    #[test]
    fn required_kem_missing_fails() {
        let mut bytes = Vec::new();
        push_tlv(
            &mut bytes,
            TYPE_PQ_KEM,
            &[KemId::MlKem1024.code(), REQUIRED_FLAG],
        );
        push_tlv(
            &mut bytes,
            TYPE_PQ_SIG,
            &[SignatureId::Dilithium3.code(), REQUIRED_FLAG],
        );
        push_tlv(&mut bytes, TYPE_TRANSCRIPT_COMMIT, &[0xAA; 32]);

        let client = parse_client_advertisement(&bytes).expect("parse client");
        let err = negotiate_capabilities(&client, &sample_server_caps()).unwrap_err();
        matches!(err, CapabilityError::RequiredKemMissing(KemId::MlKem1024));
    }

    #[test]
    fn transcript_commit_mismatch_detected() {
        let mut client = parse_client_advertisement(&sample_client_vector()).expect("parse");
        client.transcript_commit = Some([0xBB; 32]);
        let err = negotiate_capabilities(&client, &sample_server_caps()).unwrap_err();
        matches!(err, CapabilityError::TranscriptCommitMismatch);
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
        matches!(err, CapabilityError::ConstantRateStrictRequired);
    }

    #[test]
    fn constant_rate_version_mismatch_rejected() {
        let mut client = parse_client_advertisement(&sample_client_vector()).expect("parse");
        let mut upgraded = sample_constant_rate(ConstantRateMode::BestEffort);
        upgraded.version = 2;
        client.constant_rate = Some(upgraded);
        let err = negotiate_capabilities(&client, &sample_server_caps()).unwrap_err();
        matches!(err, CapabilityError::ConstantRateUnsupportedVersion(2));
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

        let relay_vector = encode_relay_advertisement(&negotiated, 0x01);
        assert!(
            relay_vector.windows(8).any(|window| {
                window
                    == [
                        (TYPE_CONSTANT_RATE >> 8) as u8,
                        (TYPE_CONSTANT_RATE & 0xFF) as u8,
                        0x04,
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
}
