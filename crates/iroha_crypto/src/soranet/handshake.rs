//! `SoraNet` handshake helpers shared across runtime components and the fixture harness.
//!
//! Provides capability TLV parsing, transcript hashing, a deterministic Noise XX
//! handshake simulation (with ML-KEM material, dual signatures, and padded
//! frames), plus helpers for salt/telemetry fixture generation.

use std::{
    convert::{TryFrom, TryInto},
    fmt, fs,
    ops::Deref,
    path::{Path, PathBuf},
    str::FromStr,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as Base64};
use ed25519_dalek::{Signer, SigningKey};
use hex::FromHex;
use hkdf::Hkdf;
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json::{self, Map, Value},
};
use rand_core::{CryptoRng, RngCore};
use sha3::{
    Digest, Sha3_256, Shake256,
    digest::{ExtendableOutput, Update, XofReader},
};
use soranet_pq::{
    MlKemMetadata, MlKemParameters, MlKemSharedSecret, MlKemSuite, decapsulate_mlkem,
    encapsulate_mlkem, generate_mlkem_keypair, mlkem_metadata, validate_mlkem_ciphertext,
    validate_mlkem_public_key,
};
use tempfile::TempDir;
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
use zeroize::Zeroizing;

use crate::KeyPair;

/// Domain separation tag for transcript hashing.
const TRANSCRIPT_DOMAIN: &[u8] = b"soranet.transcript.v1";
const STEP_DOMAIN: &[u8] = b"soranet.noise.step.v1";

const HYBRID_CLIENT_HELLO_TYPE: u8 = 0x11;
const HYBRID_RELAY_RESPONSE_TYPE: u8 = 0x12;
const PQFS_CLIENT_COMMIT_TYPE: u8 = 0x21;
const PQFS_RELAY_RESPONSE_TYPE: u8 = 0x22;
const NK2_CONFIRM_LABEL: &[u8] = b"soranet.handshake.nk2.confirm";
const NK3_PRIMARY_CONFIRM_LABEL: &[u8] = b"soranet.handshake.nk3.confirm.primary";
const NK3_FORWARD_CONFIRM_LABEL: &[u8] = b"soranet.handshake.nk3.confirm.forward";

const DILITHIUM3_SIGNATURE_LEN: usize = 2701;
const ED25519_SIGNATURE_LEN: usize = 64;
const NOISE_SECRET_LEN: usize = 32;
const NOISE_PADDING_BLOCK: usize = 1024;

const STEP_NOTE_HYBRID_INIT: &str = "Client sends NK2 hybrid init";
const STEP_NOTE_HYBRID_RESPONSE: &str = "Relay completes NK2 hybrid handshake";
const STEP_NOTE_PQFS_COMMIT: &str = "Client commits NK3 forward-secure material";
const STEP_NOTE_PQFS_RESPONSE: &str = "Relay finalises NK3 forward-secure handshake";

/// Range reserved for GREASE TLVs which should be ignored during negotiation
/// but still included in transcript hashing.
const GREASE_RANGE_START: u16 = 0x7F00;
const GREASE_RANGE_END: u16 = 0x7FFF;

const SIGNATURE_PREFIX_DILITHIUM: &str = "dilithium3";
const SIGNATURE_PREFIX_ED25519: &str = "ed25519";
const TELEMETRY_DILITHIUM_LABEL: &[u8] = b"soranet.sig.dilithium.telemetry";
const TELEMETRY_ED25519_LABEL: &[u8] = b"soranet.sig.ed25519.telemetry";
const ALARM_DILITHIUM_LABEL: &[u8] = b"soranet.sig.dilithium.alarm";
const ALARM_ED25519_LABEL: &[u8] = b"soranet.sig.ed25519.alarm";
const FIXTURE_RELAY_SIGNATURE_KEY: [u8; NOISE_SECRET_LEN] = [0x42; NOISE_SECRET_LEN];
const CAPABILITY_PQKEM: u16 = 0x0101;
const CAPABILITY_PQSIG: u16 = 0x0102;
const CAPABILITY_TRANSCRIPT_COMMIT: u16 = 0x0103;
const CAPABILITY_SUITE_LIST: u16 = 0x0104;
const CAPABILITY_ROLE: u16 = 0x0201;
const CAPABILITY_PADDING: u16 = 0x0202;
const CAPABILITY_CONSTANT_RATE: u16 = 0x0203;
const CAPABILITY_REQUIRED_FLAG: u8 = 0x01;
/// Negotiated handshake suite identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HandshakeSuite {
    /// Two-flight hybrid handshake mixing classical and PQ material.
    Nk2Hybrid = 0x02,
    /// Forward-secure PQ handshake with dual commitments.
    Nk3PqForwardSecure = 0x03,
}

impl HandshakeSuite {
    /// Canonical label used when rendering the negotiated handshake flavour.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Nk2Hybrid => "nk2.hybrid",
            Self::Nk3PqForwardSecure => "nk3.pq_forward_secure",
        }
    }
}

impl TryFrom<u8> for HandshakeSuite {
    type Error = HarnessError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x02 => Ok(Self::Nk2Hybrid),
            0x03 => Ok(Self::Nk3PqForwardSecure),
            other => Err(HarnessError::Validation(format!(
                "unsupported handshake suite identifier {other:#04x}"
            ))),
        }
    }
}

impl From<HandshakeSuite> for u8 {
    fn from(value: HandshakeSuite) -> Self {
        value as u8
    }
}

impl fmt::Display for HandshakeSuite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug)]
struct SuiteList {
    suites: Vec<HandshakeSuite>,
    #[allow(dead_code)]
    required: bool,
}

impl SuiteList {
    fn preferred(&self) -> Option<HandshakeSuite> {
        self.suites.first().copied()
    }
}

#[derive(Debug)]
struct HandshakeSuiteNegotiation {
    selected: HandshakeSuite,
    warnings: Vec<CapabilityWarning>,
}

/// Parsed capability TLV entry.
#[derive(Debug, Clone, PartialEq, Eq, JsonSerialize)]
pub struct CapabilityTlv {
    /// Numeric type identifier.
    pub ty: u16,
    /// Raw TLV payload.
    pub value: Vec<u8>,
    /// Whether the capability was marked as required by its TLV-specific flag.
    pub required: bool,
}

impl CapabilityTlv {
    /// Returns `true` when this capability occupies the GREASE-reserved range.
    pub fn is_grease(&self) -> bool {
        (GREASE_RANGE_START..=GREASE_RANGE_END).contains(&self.ty)
    }
}

/// Errors surfaced while parsing or validating the handshake inputs.
#[derive(Debug, Error)]
pub enum HarnessError {
    /// Capability TLV ended before the length/type header could be read.
    #[error("capability tlv truncated at offset {0}")]
    TlvTruncated(usize),
    /// Capability TLV declared length longer than the remaining buffer.
    #[error("capability tlv length exceeds buffer at offset {0}")]
    TlvLengthExceeded(usize),
    /// Additional bytes remained after parsing the capability vector.
    #[error("unconsumed bytes remain at end of capability vector")]
    ExtraBytes,
    /// Provided hex input failed to decode.
    #[error("invalid hex input: {0}")]
    Hex(#[from] hex::FromHexError),
    /// Salt hex string was not exactly 32 bytes.
    #[error("salt hex must decode to 32 bytes, got {0}")]
    SaltLength(usize),
    /// Supplied capability identifier was not recognised.
    #[error("invalid capability type: {0}")]
    CapabilityType(String),
    /// Duplicate capability TLV encountered for a singleton type.
    #[error("duplicate capability {0}")]
    DuplicateCapability(String),
    /// Timestamp failed to parse according to RFC3339.
    #[error("invalid timestamp {timestamp}: {source}")]
    Timestamp {
        /// Original timestamp string supplied by the caller.
        timestamp: String,
        #[source]
        /// Underlying parser error.
        source: time::error::Parse,
    },
    /// Rendering a JSON payload failed.
    #[error("failed to render JSON: {0}")]
    Json(#[from] json::Error),
    /// Placeholder for features that have not yet been implemented.
    #[error("feature not yet implemented: {0}")]
    NotImplemented(&'static str),
    /// Propagated I/O failure while reading or writing fixtures.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Validation failure occurred while building or comparing fixtures.
    #[error("capability validation error: {0}")]
    Validation(String),
    /// Mandatory capability missing or downgrade detected during negotiation.
    #[error("capability downgrade detected")]
    Downgrade {
        /// Capability warnings explaining the rejection.
        warnings: Vec<CapabilityWarning>,
        /// Optional telemetry payload describing the downgrade event.
        telemetry: Option<Vec<u8>>,
    },
    /// ML-KEM operation failed while constructing the handshake.
    #[error("ml-kem operation failed: {0}")]
    Kem(String),
    /// HKDF expansion failed while deriving confirmation or session material.
    #[error("hkdf expansion failed")]
    Kdf,
}

/// Parse a capability vector into structured TLVs.
///
/// # Errors
/// Returns an error when the TLV encoding is truncated or otherwise malformed.
pub fn parse_capabilities(buf: &[u8]) -> Result<Vec<CapabilityTlv>, HarnessError> {
    let mut offset = 0;
    let mut out = Vec::new();
    let mut seen_singletons = std::collections::BTreeSet::new();

    while offset < buf.len() {
        if offset + 4 > buf.len() {
            return Err(HarnessError::TlvTruncated(offset));
        }
        let ty = u16::from_be_bytes([buf[offset], buf[offset + 1]]);
        let len = u16::from_be_bytes([buf[offset + 2], buf[offset + 3]]) as usize;
        offset += 4;

        if offset + len > buf.len() {
            return Err(HarnessError::TlvLengthExceeded(offset));
        }
        let mut value = buf[offset..offset + len].to_vec();
        offset += len;

        let grease = (GREASE_RANGE_START..=GREASE_RANGE_END).contains(&ty);
        if !grease && !capability_is_known(ty) {
            return Err(HarnessError::CapabilityType(capability_label(ty)));
        }
        if !grease {
            validate_capability_payload(ty, &value)?;
            if !capability_allows_duplicates(ty) && !seen_singletons.insert(ty) {
                return Err(HarnessError::DuplicateCapability(capability_label(ty)));
            }
        }

        let required = parse_required_flag(ty, &mut value);

        out.push(CapabilityTlv {
            ty,
            value,
            required,
        });
    }

    if offset != buf.len() {
        return Err(HarnessError::ExtraBytes);
    }

    Ok(out)
}

/// Helper to decode hex strings supplied on the CLI.
///
/// # Errors
/// Returns an error if the input contains non-hexadecimal characters.
pub fn decode_hex(input: &str) -> Result<Vec<u8>, HarnessError> {
    Ok(Vec::from_hex(input)?)
}

/// Decode a blinded CID salt from hex into a fixed 32-byte array.
///
/// # Errors
/// Returns an error if the decoded salt does not contain exactly 32 bytes.
pub fn decode_salt_hex(input: &str) -> Result<[u8; 32], HarnessError> {
    let bytes = Vec::from_hex(input)?;
    if bytes.len() != 32 {
        return Err(HarnessError::SaltLength(bytes.len()));
    }
    let mut salt = [0u8; 32];
    salt.copy_from_slice(&bytes);
    Ok(salt)
}

fn parse_timestamp_nanos(ts: &str) -> Result<i128, HarnessError> {
    let dt = OffsetDateTime::parse(ts, &Rfc3339).map_err(|source| HarnessError::Timestamp {
        timestamp: ts.to_owned(),
        source,
    })?;
    Ok(dt.unix_timestamp_nanos())
}

fn nanos_to_i64(value: i128, field: &str) -> Result<i64, HarnessError> {
    i64::try_from(value).map_err(|_| {
        HarnessError::Validation(format!(
            "{field} timestamp {value} is outside the supported i64 range for JSON"
        ))
    })
}

fn option_str_to_value(value: Option<&str>) -> Value {
    value.map_or(Value::Null, Value::from)
}

fn capability_name(ty: u16) -> Option<&'static str> {
    match ty {
        CAPABILITY_PQKEM => Some("snnet.pqkem"),
        CAPABILITY_PQSIG => Some("snnet.pqsig"),
        CAPABILITY_TRANSCRIPT_COMMIT => Some("snnet.transcript_commit"),
        CAPABILITY_SUITE_LIST => Some("snnet.suite_list"),
        CAPABILITY_ROLE => Some("snnet.role"),
        CAPABILITY_PADDING => Some("snnet.padding"),
        CAPABILITY_CONSTANT_RATE => Some("snnet.constant_rate"),
        _ => None,
    }
}

fn capability_label(ty: u16) -> String {
    capability_name(ty).map_or_else(
        || format!("type=0x{ty:04x}"),
        |name| format!("type=0x{ty:04x} ({name})"),
    )
}

fn capability_is_known(ty: u16) -> bool {
    matches!(
        ty,
        CAPABILITY_PQKEM
            | CAPABILITY_PQSIG
            | CAPABILITY_TRANSCRIPT_COMMIT
            | CAPABILITY_SUITE_LIST
            | CAPABILITY_ROLE
            | CAPABILITY_PADDING
            | CAPABILITY_CONSTANT_RATE
    )
}

fn capability_allows_duplicates(ty: u16) -> bool {
    matches!(ty, CAPABILITY_PQKEM | CAPABILITY_PQSIG)
}

fn validate_capability_payload(ty: u16, value: &[u8]) -> Result<(), HarnessError> {
    match ty {
        CAPABILITY_PQKEM => {
            if value.len() != 2 {
                return Err(HarnessError::Validation(format!(
                    "snnet.pqkem capability must be 2 bytes, got {}",
                    value.len()
                )));
            }
        }
        CAPABILITY_PQSIG => {
            if value.len() != 2 {
                return Err(HarnessError::Validation(format!(
                    "snnet.pqsig capability must be 2 bytes, got {}",
                    value.len()
                )));
            }
        }
        CAPABILITY_TRANSCRIPT_COMMIT => {
            if value.len() != 32 {
                return Err(HarnessError::Validation(format!(
                    "snnet.transcript_commit capability must be 32 bytes, got {}",
                    value.len()
                )));
            }
        }
        CAPABILITY_SUITE_LIST => {
            if value.is_empty() {
                return Err(HarnessError::Validation(
                    "suite_list capability must contain at least one identifier".into(),
                ));
            }
        }
        CAPABILITY_ROLE => {
            if value.len() != 1 {
                return Err(HarnessError::Validation(format!(
                    "snnet.role capability must be 1 byte, got {}",
                    value.len()
                )));
            }
        }
        CAPABILITY_PADDING => {
            if value.len() != 2 {
                return Err(HarnessError::Validation(format!(
                    "snnet.padding capability must be 2 bytes, got {}",
                    value.len()
                )));
            }
        }
        CAPABILITY_CONSTANT_RATE => {
            if value.len() < 2 {
                return Err(HarnessError::Validation(format!(
                    "snnet.constant_rate capability must be at least 2 bytes, got {}",
                    value.len()
                )));
            }
        }
        _ => {}
    }
    Ok(())
}

fn parse_required_flag(ty: u16, value: &mut [u8]) -> bool {
    match ty {
        CAPABILITY_SUITE_LIST => {
            value.first_mut().is_some_and(|first| {
                let required = (*first & 0x80) != 0;
                *first &= 0x7F;
                required
            })
        }
        CAPABILITY_PQKEM | CAPABILITY_PQSIG | CAPABILITY_CONSTANT_RATE => value
            .get(1)
            .is_some_and(|flags| (flags & CAPABILITY_REQUIRED_FLAG) != 0),
        _ => false,
    }
}

fn apply_required_flag(ty: u16, value: &mut [u8], required: bool) {
    match ty {
        CAPABILITY_SUITE_LIST => {
            if let Some(first) = value.first_mut() {
                if required {
                    *first |= 0x80;
                } else {
                    *first &= 0x7F;
                }
            }
        }
        CAPABILITY_PQKEM | CAPABILITY_PQSIG | CAPABILITY_CONSTANT_RATE => {
            if let Some(flags) = value.get_mut(1) {
                if required {
                    *flags |= CAPABILITY_REQUIRED_FLAG;
                } else {
                    *flags &= !CAPABILITY_REQUIRED_FLAG;
                }
            }
        }
        _ => {}
    }
}

fn parse_suite_list(cap: &CapabilityTlv) -> Result<SuiteList, HarnessError> {
    if cap.value.is_empty() {
        return Err(HarnessError::Validation(
            "suite_list capability must contain at least one identifier".into(),
        ));
    }
    let mut suites = Vec::with_capacity(cap.value.len());
    let mut ignored = Vec::new();
    for &id in &cap.value {
        // Ignore unknown suite identifiers for forward compatibility.
        match HandshakeSuite::try_from(id) {
            Ok(suite) => {
                if suites.contains(&suite) {
                    continue;
                }
                suites.push(suite);
            }
            Err(_) => ignored.push(id),
        }
    }
    if suites.is_empty() {
        let unsupported = ignored
            .iter()
            .map(|id| format!("{id:#04x}"))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(HarnessError::Validation(format!(
            "suite_list capability must include at least one supported identifier; got {unsupported}"
        )));
    }
    Ok(SuiteList {
        suites,
        required: cap.required,
    })
}

fn suite_list_capability(caps: &[CapabilityTlv]) -> Result<Option<SuiteList>, HarnessError> {
    caps.iter()
        .find(|cap| cap.ty == CAPABILITY_SUITE_LIST)
        .map(parse_suite_list)
        .transpose()
}

fn suite_warning(message: impl Into<String>) -> CapabilityWarning {
    CapabilityWarning {
        capability_type: CAPABILITY_SUITE_LIST,
        message: message.into(),
    }
}

fn describe_suites(suites: &[HandshakeSuite]) -> String {
    suites
        .iter()
        .map(|suite| suite.label())
        .collect::<Vec<_>>()
        .join(", ")
}

fn handle_both_suite_lists(
    client: &SuiteList,
    relay: &SuiteList,
) -> Result<HandshakeSuiteNegotiation, HarnessError> {
    let mut negotiated = None;
    for suite in &client.suites {
        if relay.suites.contains(suite) {
            negotiated = Some(*suite);
            break;
        }
    }
    let selected = negotiated.ok_or_else(|| HarnessError::Downgrade {
        warnings: vec![suite_warning(format!(
            "no overlapping handshake suite between client ({}) and relay ({})",
            describe_suites(&client.suites),
            describe_suites(&relay.suites)
        ))],
        telemetry: None,
    })?;

    let mut warnings = Vec::new();
    if let Some(preferred) = client.preferred()
        && preferred != selected
    {
        warnings.push(suite_warning(format!(
            "client preferred {} but negotiated {}",
            preferred.label(),
            selected.label()
        )));
    }
    if let Some(preferred) = relay.preferred()
        && preferred != selected
    {
        warnings.push(suite_warning(format!(
            "relay preferred {} but negotiated {}",
            preferred.label(),
            selected.label()
        )));
    }

    Ok(HandshakeSuiteNegotiation { selected, warnings })
}

fn negotiate_handshake_suite(
    client_caps: &[CapabilityTlv],
    relay_caps: &[CapabilityTlv],
) -> Result<HandshakeSuiteNegotiation, HarnessError> {
    let client_list = suite_list_capability(client_caps)?;
    let relay_list = suite_list_capability(relay_caps)?;

    match (client_list, relay_list) {
        (Some(client), Some(relay)) => handle_both_suite_lists(&client, &relay),
        (Some(client), None) => Err(HarnessError::Downgrade {
            warnings: vec![suite_warning(format!(
                "relay omitted suite_list capability; client advertised {}",
                describe_suites(&client.suites)
            ))],
            telemetry: None,
        }),
        (None, Some(relay)) => Err(HarnessError::Downgrade {
            warnings: vec![suite_warning(format!(
                "client omitted suite_list capability; relay advertised {}",
                describe_suites(&relay.suites)
            ))],
            telemetry: None,
        }),
        (None, None) => Err(HarnessError::Downgrade {
            warnings: vec![suite_warning(
                "suite_list capability is required for handshake negotiation",
            )],
            telemetry: None,
        }),
    }
}

fn encode_signature(prefix: &str, bytes: &[u8]) -> String {
    let encoded_len = 4 * bytes.len().div_ceil(3);
    let mut buffer = vec![0u8; encoded_len];
    let written = Base64
        .encode_slice(bytes, &mut buffer)
        .expect("base64 buffer length must be sufficient");
    buffer.truncate(written);
    let encoded = String::from_utf8(buffer).expect("base64 encoding produced invalid UTF-8");
    format!("{prefix}:{encoded}")
}

fn signature_pair_from_static(
    static_key: &[u8],
    payload: &[u8],
    dilithium_label: &[u8],
    ed25519_label: &[u8],
) -> Result<(String, String), HarnessError> {
    let dilithium = expand_material(
        dilithium_label,
        &[static_key, payload],
        DILITHIUM3_SIGNATURE_LEN,
    );
    let ed = derive_ed25519_signature(ed25519_label, static_key, &[payload])?;
    Ok((
        encode_signature(SIGNATURE_PREFIX_DILITHIUM, &dilithium),
        encode_signature(SIGNATURE_PREFIX_ED25519, ed.as_ref()),
    ))
}

fn alarm_payload_map(spec: &FixtureSpec, alarm: AlarmSpec) -> Map {
    let mut map = Map::new();
    map.insert("relay_id".to_string(), Value::from(alarm.relay_id));
    map.insert("relay_role".to_string(), Value::from(spec.relay_role));
    map.insert("client_id".to_string(), Value::Null);
    map.insert(
        "capability_type".to_string(),
        Value::from(alarm.capability_type),
    );
    map.insert(
        "capability_value_hex".to_string(),
        alarm.capability_value_hex.map_or(Value::Null, Value::from),
    );
    map.insert("reason_code".to_string(), Value::from(alarm.reason));
    map.insert(
        "transcript_hash_hex".to_string(),
        Value::from(spec.transcript_hash_hex),
    );
    map.insert(
        "observed_at_unix_ms".to_string(),
        Value::from(1_785_120_000_000u64),
    );
    map.insert("circuit_id".to_string(), Value::from(20_882u64));
    map.insert("counter".to_string(), Value::from(1u32));
    map
}

fn signed_alarm_value(
    spec: &FixtureSpec,
    alarm: AlarmSpec,
    static_key: &[u8],
) -> Result<Value, HarnessError> {
    let mut map = alarm_payload_map(spec, alarm);
    let payload_value = Value::Object(map.clone());
    let payload_bytes = norito::json::to_vec(&payload_value)?;
    let (signature, witness) = signature_pair_from_static(
        static_key,
        &payload_bytes,
        ALARM_DILITHIUM_LABEL,
        ALARM_ED25519_LABEL,
    )?;
    map.insert("signature".to_string(), Value::from(signature));
    map.insert("witness_signature".to_string(), Value::from(witness));
    Ok(Value::Object(map))
}

/// Compare client and relay capability vectors and produce warnings for missing
/// required entries. GREASE capabilities are ignored.
pub fn diff_capabilities(
    client: &[CapabilityTlv],
    relay: &[CapabilityTlv],
) -> Vec<CapabilityWarning> {
    let mut warnings = Vec::new();
    for cap in client {
        if cap.is_grease() {
            continue;
        }
        if !cap.required {
            continue;
        }
        let supported = relay.iter().any(|r| r.ty == cap.ty && r.value == cap.value);
        if !supported {
            warnings.push(CapabilityWarning {
                capability_type: cap.ty,
                message: format!(
                    "relay missing required capability {}",
                    capability_label(cap.ty)
                ),
            });
        }
    }
    warnings
}

fn encode_capabilities(entries: &[CapabilityTlv]) -> Vec<u8> {
    let mut buf = Vec::new();
    for cap in entries {
        let mut value = cap.value.clone();
        apply_required_flag(cap.ty, &mut value, cap.required);
        buf.extend_from_slice(&cap.ty.to_be_bytes());
        let len = u16::try_from(value.len()).expect("capability value fits u16");
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&value);
    }
    buf
}

/// Return a capability vector based on `base` with the suite list replaced by `suites`.
///
/// # Errors
/// Returns [`HarnessError::Validation`] when `suites` is empty or the capability stream is malformed.
pub fn update_suite_list(
    base: &[u8],
    suites: &[HandshakeSuite],
    required: bool,
) -> Result<Vec<u8>, HarnessError> {
    if suites.is_empty() {
        return Err(HarnessError::Validation(
            "interop suite list requires at least one entry".into(),
        ));
    }
    let mut entries = parse_capabilities(base)?;
    let mut value = Vec::with_capacity(suites.len());
    for suite in suites {
        value.push(u8::from(*suite));
    }
    if let Some(existing) = entries
        .iter_mut()
        .find(|cap| cap.ty == CAPABILITY_SUITE_LIST)
    {
        existing.value = value;
        existing.required = required;
    } else {
        entries.push(CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value,
            required,
        });
    }
    entries.sort_by_key(|cap| cap.ty);
    Ok(encode_capabilities(&entries))
}

/// Transcript hash inputs for the Noise XX handshake.
#[derive(Debug)]
pub struct TranscriptInputs<'a> {
    /// Merkle commitment for the relay descriptor.
    pub descriptor_commit: &'a [u8],
    /// Client-generated nonce advertised during the handshake.
    pub client_nonce: &'a [u8],
    /// Relay-generated nonce advertised during the handshake.
    pub relay_nonce: &'a [u8],
    /// Raw capability TLVs serialized for hashing.
    pub capability_bytes: &'a [u8],
    /// KEM identifier selected for the session.
    pub kem_id: u8,
    /// Signature suite identifier selected for the session.
    pub sig_id: u8,
    /// Handshake suite identifier negotiated for the session.
    pub handshake_suite: HandshakeSuite,
    /// Optional resume hash supplied when resuming a session.
    pub resume_hash: Option<&'a [u8]>,
}

impl TranscriptInputs<'_> {
    /// Compute the transcript hash as described in the working draft.
    pub fn compute_hash(&self) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        Update::update(&mut hasher, TRANSCRIPT_DOMAIN);

        let desc_hash = Sha3_256::digest(self.descriptor_commit);
        Update::update(&mut hasher, desc_hash.as_ref());
        Update::update(&mut hasher, self.client_nonce);
        Update::update(&mut hasher, self.relay_nonce);

        let cap_len = u32::try_from(self.capability_bytes.len())
            .expect("capability vector length fits u32")
            .to_be_bytes();
        Update::update(&mut hasher, &cap_len);
        Update::update(&mut hasher, self.capability_bytes);

        Update::update(&mut hasher, &[self.kem_id]);
        Update::update(&mut hasher, &[self.sig_id]);
        Update::update(&mut hasher, &[self.handshake_suite as u8]);

        if let Some(resume) = self.resume_hash {
            Update::update(&mut hasher, resume);
        }

        hasher.finalize().into()
    }
}

/// Utility for pretty-printing capability TLVs.
pub fn format_capabilities(caps: &[CapabilityTlv]) -> impl fmt::Display + '_ {
    struct Formatter<'a>(&'a [CapabilityTlv]);

    impl fmt::Display for Formatter<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for cap in self.0 {
                writeln!(
                    f,
                    "type=0x{ty:04x}, len={len}, required={required}, grease={grease}",
                    ty = cap.ty,
                    len = cap.value.len(),
                    required = cap.required,
                    grease = cap.is_grease()
                )?;
            }
            Ok(())
        }
    }

    Formatter(caps)
}

/// Helper structure for serialising capability summaries to JSON fixtures.
#[derive(Debug, JsonSerialize)]
/// JSON-friendly summary of capability TLVs.
pub struct CapabilitySummary<'a> {
    /// Capability entries to encode.
    pub tlvs: &'a [CapabilityTlv],
}

impl CapabilitySummary<'_> {
    /// Convert the TLV summary into human-readable JSON.
    ///
    /// # Errors
    /// Returns an error when a capability cannot be serialized to JSON.
    pub fn to_pretty_json(&self) -> Result<String, json::Error> {
        let mut map = Map::new();
        let tlvs = self
            .tlvs
            .iter()
            .map(json::to_value)
            .collect::<Result<Vec<_>, _>>()?;
        map.insert("tlvs".to_string(), Value::from(tlvs));
        json::to_string_pretty(&Value::from(map))
    }
}

/// Convenience type for passing optional hex on the CLI.
/// Newtype wrapper for hex-decoded CLI inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexInput(pub Vec<u8>);

impl FromStr for HexInput {
    type Err = HarnessError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(HexInput(decode_hex(s)?))
    }
}

/// Parameters for generating a salt announcement JSON payload.
#[derive(Debug, Copy, Clone)]
pub struct SaltAnnouncementParams<'a> {
    /// Epoch identifier for the announcement.
    pub epoch_id: u32,
    /// Optional previous epoch this announcement supersedes.
    pub previous_epoch: Option<u32>,
    /// ISO8601 timestamp after which the salt becomes valid.
    pub valid_after: &'a str,
    /// ISO8601 timestamp after which the salt expires.
    pub valid_until: &'a str,
    /// Blinded CID salt advertised to clients.
    pub blinded_cid_salt: &'a [u8; 32],
    /// Whether the announcement is triggered by an emergency rotation.
    pub emergency_rotation: bool,
    /// Optional operator notes associated with the announcement.
    pub notes: Option<&'a str>,
    /// Optional signature covering the announcement.
    pub signature: Option<&'a str>,
}

#[derive(Debug, JsonSerialize)]
struct SaltAnnouncement<'a> {
    epoch_id: u32,
    previous_epoch: Option<u32>,
    valid_after: i64,
    valid_until: i64,
    blinded_cid_salt_hex: String,
    emergency_rotation: bool,
    notes: Option<&'a str>,
    signature: Option<&'a str>,
}

/// Render a pretty-printed JSON `SaltAnnouncement` payload from the provided parameters.
///
/// # Errors
/// Returns an error when timestamp parsing or JSON serialization fails.
pub fn salt_announcement_json(params: &SaltAnnouncementParams<'_>) -> Result<String, HarnessError> {
    let valid_after = nanos_to_i64(parse_timestamp_nanos(params.valid_after)?, "valid_after")?;
    let valid_until = nanos_to_i64(parse_timestamp_nanos(params.valid_until)?, "valid_until")?;
    let payload = SaltAnnouncement {
        epoch_id: params.epoch_id,
        previous_epoch: params.previous_epoch,
        valid_after,
        valid_until,
        blinded_cid_salt_hex: hex::encode(params.blinded_cid_salt),
        emergency_rotation: params.emergency_rotation,
        notes: params.notes,
        signature: params.signature,
    };
    let value = json::to_value(&payload)?;
    Ok(json::to_string_pretty(&value)?)
}

#[derive(Debug, JsonDeserialize)]
struct SaltAnnouncementRecord {
    epoch_id: u32,
    previous_epoch: Option<u32>,
    valid_after: i64,
    valid_until: i64,
    blinded_cid_salt_hex: String,
    emergency_rotation: bool,
    notes: Option<String>,
    signature: Option<String>,
}

/// Result of verifying a salt announcement fixture.
#[derive(Debug)]
pub struct SaltAnnouncementValidation {
    /// Epoch identifier extracted from the announcement.
    pub epoch_id: u32,
    /// Optional epoch expected to precede this announcement.
    pub previous_epoch: Option<u32>,
    /// Start of validity window (unix timestamp in nanoseconds).
    pub valid_after: i64,
    /// End of validity window (unix timestamp in nanoseconds).
    pub valid_until: i64,
    /// Indicates whether a signature field was present.
    pub has_signature: bool,
    /// Whether the announcement was flagged as an emergency rotation.
    pub emergency_rotation: bool,
    /// Optional operator notes included in the announcement.
    pub notes: Option<String>,
}

/// Verify a `SaltAnnouncement` fixture stored on disk.
///
/// # Errors
/// Returns an error if the fixture cannot be parsed or fails validation checks.
pub fn verify_salt_vector(path: &Path) -> Result<SaltAnnouncementValidation, HarnessError> {
    let contents = fs::read_to_string(path)?;
    let record: SaltAnnouncementRecord = json::from_str(&contents)?;

    decode_salt_hex(&record.blinded_cid_salt_hex).map_err(|err| match err {
        HarnessError::SaltLength(len) => HarnessError::Validation(format!(
            "salt fixture {}: blinded_cid_salt_hex must decode to 32 bytes, got {len}",
            path.display()
        )),
        other => other,
    })?;

    if record.valid_after >= record.valid_until {
        return Err(HarnessError::Validation(format!(
            "salt fixture {}: valid_until ({}) must be greater than valid_after ({})",
            path.display(),
            record.valid_until,
            record.valid_after
        )));
    }

    if let Some(prev) = record.previous_epoch
        && prev.checked_add(1) != Some(record.epoch_id)
    {
        return Err(HarnessError::Validation(format!(
            "salt fixture {}: previous_epoch ({prev}) must precede epoch_id ({}) by 1",
            path.display(),
            record.epoch_id
        )));
    }

    let has_signature = record
        .signature
        .as_ref()
        .is_some_and(|sig| !sig.trim().is_empty());

    Ok(SaltAnnouncementValidation {
        epoch_id: record.epoch_id,
        previous_epoch: record.previous_epoch,
        valid_after: record.valid_after,
        valid_until: record.valid_until,
        has_signature,
        emergency_rotation: record.emergency_rotation,
        notes: record.notes,
    })
}

/// Alarm report emitted when capability negotiation detects misuse.
#[derive(Debug, JsonSerialize)]
pub struct AlarmReport<'a> {
    /// Identifier of the relay raising the alarm.
    pub relay_id: &'a str,
    /// Numerical role indicator for the relay.
    pub relay_role: u8,
    /// Capability type that triggered the alarm.
    pub capability_type: u16,
    /// Reason code summarising the violation.
    pub reason_code: &'a str,
    /// Transcript hash associated with the offending session (hex encoded).
    pub transcript_hash_hex: &'a str,
    /// Timestamp when the alarm was observed (milliseconds since Unix epoch).
    pub observed_at_unix_ms: u64,
    /// Circuit identifier the session belonged to.
    pub circuit_id: u64,
    /// Monotonic alarm counter for the relay.
    pub counter: u32,
    /// Optional capability payload associated with the alarm (hex encoded).
    pub capability_value_hex: Option<&'a str>,
    /// Optional relay signature covering the report payload.
    pub signature: Option<&'a str>,
    /// Optional witness signature accompanying the alarm.
    pub witness_signature: Option<&'a str>,
}

/// Render an alarm report into a canonical JSON value.
///
/// # Errors
/// Returns an error if the JSON serialization fails.
pub fn alarm_report_json(report: &AlarmReport<'_>) -> Result<String, HarnessError> {
    let mut map = Map::new();
    map.insert("relay_id".to_string(), Value::from(report.relay_id));
    map.insert("relay_role".to_string(), Value::from(report.relay_role));
    map.insert(
        "capability_type".to_string(),
        Value::from(report.capability_type),
    );
    map.insert("reason_code".to_string(), Value::from(report.reason_code));
    map.insert(
        "transcript_hash_hex".to_string(),
        Value::from(report.transcript_hash_hex),
    );
    map.insert(
        "observed_at_unix_ms".to_string(),
        Value::from(report.observed_at_unix_ms),
    );
    map.insert("circuit_id".to_string(), Value::from(report.circuit_id));
    map.insert("counter".to_string(), Value::from(report.counter));
    map.insert(
        "capability_value_hex".to_string(),
        report.capability_value_hex.map_or(Value::Null, Value::from),
    );
    map.insert("client_id".to_string(), Value::Null);
    map.insert(
        "signature".to_string(),
        report.signature.map_or(Value::Null, Value::from),
    );
    map.insert(
        "witness_signature".to_string(),
        report.witness_signature.map_or(Value::Null, Value::from),
    );
    Ok(json::to_string_pretty(&Value::from(map))?)
}

/// Telemetry report summarising recent relay performance.
#[derive(Debug)]
pub struct TelemetryReport<'a> {
    /// Epoch identifier associated with the statistics.
    pub epoch: u32,
    /// Number of downgrade attempts observed during the epoch.
    pub downgrade_attempts: u32,
    /// Sessions where post-quantum capability was disabled.
    pub pq_disabled_sessions: u32,
    /// Ratio of cover traffic compared to regular flows.
    pub cover_ratio: f32,
    /// Number of clients considered to be lagging behind consensus.
    pub lagging_clients: u32,
    /// Maximum observed round-trip latency in milliseconds.
    pub max_latency_ms: u32,
    /// Optional incident reference identifier.
    pub incident_reference: Option<&'a str>,
    /// Optional signature authorising the telemetry packet.
    pub signature: Option<&'a str>,
    /// Optional witness signature attached to the telemetry packet.
    pub witness_signature: Option<&'a str>,
}

/// Render a `SoraNet` telemetry payload as pretty JSON, optionally signing it with a relay secret.
///
/// # Errors
/// Returns an error if JSON serialization or signature generation fails.
pub fn soranet_telemetry_json(
    report: &TelemetryReport<'_>,
    signing_key: Option<&[u8; NOISE_SECRET_LEN]>,
) -> Result<String, HarnessError> {
    let mut map = Map::new();
    map.insert("epoch".to_string(), Value::from(report.epoch));
    map.insert(
        "downgrade_attempts".to_string(),
        Value::from(report.downgrade_attempts),
    );
    map.insert(
        "pq_disabled_sessions".to_string(),
        Value::from(report.pq_disabled_sessions),
    );
    map.insert(
        "cover_ratio".to_string(),
        Value::from(f64::from(report.cover_ratio)),
    );
    map.insert(
        "lagging_clients".to_string(),
        Value::from(report.lagging_clients),
    );
    map.insert(
        "max_latency_ms".to_string(),
        Value::from(report.max_latency_ms),
    );
    map.insert(
        "incident_reference".to_string(),
        option_str_to_value(report.incident_reference),
    );

    if let Some(key) = signing_key {
        let payload_value = Value::Object(map.clone());
        let payload_bytes = json::to_vec(&payload_value)?;
        let (signature, witness) = signature_pair_from_static(
            key,
            &payload_bytes,
            TELEMETRY_DILITHIUM_LABEL,
            TELEMETRY_ED25519_LABEL,
        )?;
        map.insert("signature".to_string(), Value::from(signature));
        map.insert("witness_signature".to_string(), Value::from(witness));
    } else {
        map.insert(
            "signature".to_string(),
            option_str_to_value(report.signature),
        );
        map.insert(
            "witness_signature".to_string(),
            option_str_to_value(report.witness_signature),
        );
    }

    Ok(json::to_string_pretty(&Value::Object(map))?)
}

#[derive(Debug, Clone, JsonSerialize)]
/// Recorded handshake message exchanged between client and relay.
pub struct HandshakeStep {
    /// Participant role that emitted the message (client or relay).
    pub role: &'static str,
    /// High-level action performed during this step.
    pub action: &'static str,
    /// Hex-encoded Noise message payload.
    pub message_hex: String,
    /// Additional notes describing the step.
    pub note: &'static str,
}

impl HandshakeStep {
    fn new(role: &'static str, action: &'static str, note: &'static str, message: Vec<u8>) -> Self {
        Self {
            role,
            action,
            message_hex: hex::encode(message),
            note,
        }
    }
}

#[derive(Debug, Clone, JsonSerialize)]
/// Warning emitted when capability negotiation detects mismatches.
pub struct CapabilityWarning {
    /// Identifier of the capability that triggered the warning.
    pub capability_type: u16,
    /// Human-readable warning message.
    pub message: String,
}

/// Output captured from a simulated handshake run.
#[derive(Debug)]
/// Artifacts produced by a deterministic handshake simulation.
pub struct SimulationResult {
    /// SHA3-256 transcript hash computed for the handshake.
    pub transcript_hash: [u8; 32],
    /// Capability compatibility warnings surfaced during the run.
    pub warnings: Vec<CapabilityWarning>,
    /// Negotiated handshake suite for the simulation.
    pub handshake_suite: HandshakeSuite,
    /// Telemetry payloads generated by the relay during the handshake.
    pub telemetry_payloads: Vec<Vec<u8>>,
    /// Parsed client capability TLVs.
    pub client_capabilities: Vec<CapabilityTlv>,
    /// Parsed relay capability TLVs.
    pub relay_capabilities: Vec<CapabilityTlv>,
    /// Chronological record of handshake steps.
    pub handshake_steps: Vec<HandshakeStep>,
}

/// Input parameters for performing a deterministic handshake simulation.
#[derive(Debug)]
pub struct SimulationParams<'a> {
    /// Serialized client capability TLVs.
    pub client_capabilities: &'a [u8],
    /// Serialized relay capability TLVs.
    pub relay_capabilities: &'a [u8],
    /// Client static secret key bytes.
    pub client_static_sk: &'a [u8],
    /// Relay static secret key bytes.
    pub relay_static_sk: &'a [u8],
    /// Optional resume hash from a previous session.
    pub resume_hash: Option<&'a [u8]>,
    /// Commitment to the relay descriptor used for transcript hashing.
    pub descriptor_commit: &'a [u8],
    /// Client nonce used during negotiation.
    pub client_nonce: &'a [u8],
    /// Relay nonce used during negotiation.
    pub relay_nonce: &'a [u8],
    /// Selected KEM identifier.
    pub kem_id: u8,
    /// Selected signature suite identifier.
    pub sig_id: u8,
}

/// Execute a deterministic Noise XX handshake simulation.
///
/// # Errors
/// Returns an error if key validation, capability parsing, or artifact generation fails.
pub fn simulate_handshake(params: &SimulationParams<'_>) -> Result<SimulationResult, HarnessError> {
    let client_caps = parse_capabilities(params.client_capabilities)?;
    let relay_caps = parse_capabilities(params.relay_capabilities)?;
    let HandshakeSuiteNegotiation {
        selected: handshake_suite,
        mut warnings,
    } = negotiate_handshake_suite(&client_caps, &relay_caps)?;
    warnings.extend(diff_capabilities(&client_caps, &relay_caps));

    let client_static = validate_static_key("client", params.client_static_sk)?;
    let relay_static = validate_static_key("relay", params.relay_static_sk)?;

    let transcript_hash = TranscriptInputs {
        descriptor_commit: params.descriptor_commit,
        client_nonce: params.client_nonce,
        relay_nonce: params.relay_nonce,
        capability_bytes: params.client_capabilities,
        kem_id: params.kem_id,
        sig_id: params.sig_id,
        handshake_suite,
        resume_hash: params.resume_hash,
    }
    .compute_hash();

    let artifacts = build_handshake_artifacts(
        params,
        &client_static,
        &relay_static,
        &transcript_hash,
        handshake_suite,
        &warnings,
    )?;

    Ok(SimulationResult {
        transcript_hash,
        warnings,
        handshake_suite,
        telemetry_payloads: artifacts.telemetry_payloads,
        client_capabilities: client_caps,
        relay_capabilities: relay_caps,
        handshake_steps: artifacts.steps,
    })
}

struct HandshakeArtifacts {
    steps: Vec<HandshakeStep>,
    telemetry_payloads: Vec<Vec<u8>>,
}

struct KemProfile {
    metadata: MlKemMetadata,
}

impl KemProfile {
    fn suite(&self) -> MlKemSuite {
        self.metadata.suite
    }

    fn parameters(&self) -> MlKemParameters {
        self.metadata.parameters
    }
}

struct SimulatedKemArtifacts {
    client_public: Vec<u8>,
    relay_public: Vec<u8>,
    ciphertext: Vec<u8>,
    confirmation: Vec<u8>,
    shared_secret: Vec<u8>,
}

#[derive(Clone, Copy)]
enum KemVariant {
    Primary,
    ForwardSecure,
}

struct KemLabelSet {
    client: &'static [u8],
    relay: &'static [u8],
    ciphertext: &'static [u8],
    confirmation: &'static [u8],
    shared: &'static [u8],
}

struct DeterministicHandshakeMaterial {
    client_static_bytes: [u8; NOISE_SECRET_LEN],
    client_static_public: [u8; NOISE_SECRET_LEN],
    client_ephemeral_public: [u8; NOISE_SECRET_LEN],
    relay_static_bytes: [u8; NOISE_SECRET_LEN],
    relay_static_public: [u8; NOISE_SECRET_LEN],
    relay_ephemeral_public: [u8; NOISE_SECRET_LEN],
    primary_kem: SimulatedKemArtifacts,
    forward_secure_kem: SimulatedKemArtifacts,
}

fn derive_kem_artifacts(
    variant: KemVariant,
    params: &SimulationParams<'_>,
    client_static: &[u8; NOISE_SECRET_LEN],
    relay_static: &[u8; NOISE_SECRET_LEN],
    profile: &KemProfile,
) -> SimulatedKemArtifacts {
    let labels = match variant {
        KemVariant::Primary => KemLabelSet {
            client: b"soranet.kem.client.public",
            relay: b"soranet.kem.relay.public",
            ciphertext: b"soranet.kem.ciphertext",
            confirmation: b"soranet.kem.confirmation",
            shared: b"soranet.kem.shared",
        },
        KemVariant::ForwardSecure => KemLabelSet {
            client: b"soranet.kem.fs.client.public",
            relay: b"soranet.kem.fs.relay.public",
            ciphertext: b"soranet.kem.fs.ciphertext",
            confirmation: b"soranet.kem.fs.confirmation",
            shared: b"soranet.kem.fs.shared",
        },
    };

    let kem_params = profile.parameters();

    let client_public = expand_material(
        labels.client,
        &[client_static.as_ref(), params.client_nonce],
        kem_params.public_key,
    );
    let relay_public = expand_material(
        labels.relay,
        &[relay_static.as_ref(), params.relay_nonce],
        kem_params.public_key,
    );
    let ciphertext = expand_material(
        labels.ciphertext,
        &[
            relay_static.as_ref(),
            relay_public.as_slice(),
            client_public.as_slice(),
        ],
        kem_params.ciphertext,
    );
    let confirmation = expand_material(
        labels.confirmation,
        &[
            client_static.as_ref(),
            client_public.as_slice(),
            relay_public.as_slice(),
        ],
        kem_params.shared_secret,
    );
    let shared_secret = expand_material(
        labels.shared,
        &[
            client_static.as_ref(),
            relay_static.as_ref(),
            client_public.as_slice(),
            relay_public.as_slice(),
        ],
        kem_params.shared_secret,
    );

    SimulatedKemArtifacts {
        client_public,
        relay_public,
        ciphertext,
        confirmation,
        shared_secret,
    }
}

fn derive_handshake_material(
    params: &SimulationParams<'_>,
    client_static: &[u8; NOISE_SECRET_LEN],
    relay_static: &[u8; NOISE_SECRET_LEN],
) -> Result<DeterministicHandshakeMaterial, HarnessError> {
    let profile = kem_profile(params.kem_id)?;
    let client_static_public =
        X25519PublicKey::from(&StaticSecret::from(*client_static)).to_bytes();
    let relay_static_public = X25519PublicKey::from(&StaticSecret::from(*relay_static)).to_bytes();

    let client_ephemeral_public = {
        let secret = StaticSecret::from(derive_seed(
            b"soranet.noise.client.ephemeral",
            &[client_static.as_ref(), params.client_nonce],
        )?);
        X25519PublicKey::from(&secret).to_bytes()
    };
    let relay_ephemeral_public = {
        let secret = StaticSecret::from(derive_seed(
            b"soranet.noise.relay.ephemeral",
            &[relay_static.as_ref(), params.relay_nonce],
        )?);
        X25519PublicKey::from(&secret).to_bytes()
    };

    let primary_kem = derive_kem_artifacts(
        KemVariant::Primary,
        params,
        client_static,
        relay_static,
        &profile,
    );
    let forward_secure_kem = derive_kem_artifacts(
        KemVariant::ForwardSecure,
        params,
        client_static,
        relay_static,
        &profile,
    );

    Ok(DeterministicHandshakeMaterial {
        client_static_bytes: *client_static,
        client_static_public,
        client_ephemeral_public,
        relay_static_bytes: *relay_static,
        relay_static_public,
        relay_ephemeral_public,
        primary_kem,
        forward_secure_kem,
    })
}

#[allow(clippy::too_many_lines)]
fn build_nk2_artifacts(
    params: &SimulationParams<'_>,
    transcript_hash: &[u8; 32],
    material: &DeterministicHandshakeMaterial,
    warnings: &[CapabilityWarning],
) -> Result<HandshakeArtifacts, HarnessError> {
    let primary = &material.primary_kem;

    let mut client_init = Vec::new();
    client_init.push(HYBRID_CLIENT_HELLO_TYPE);
    append_len_prefixed(&mut client_init, params.client_nonce);
    client_init.push(u8::from(HandshakeSuite::Nk2Hybrid));
    client_init.push(params.kem_id);
    client_init.push(params.sig_id);
    client_init.extend_from_slice(&material.client_ephemeral_public);
    append_len_prefixed(&mut client_init, &material.client_static_public);
    append_len_prefixed(&mut client_init, &primary.client_public);
    append_len_prefixed(&mut client_init, params.client_capabilities);
    match params.resume_hash {
        Some(hash) => {
            client_init.push(1);
            append_len_prefixed(&mut client_init, hash);
        }
        None => client_init.push(0),
    }
    let client_body = client_init.clone();
    let client_dilithium = expand_material(
        b"soranet.sig.dilithium.hybrid.client",
        &[
            material.client_static_bytes.as_ref(),
            client_body.as_slice(),
            transcript_hash,
        ],
        DILITHIUM3_SIGNATURE_LEN,
    );
    append_len_prefixed(&mut client_init, &client_dilithium);
    let client_ed = derive_ed25519_signature(
        b"soranet.sig.ed25519.hybrid.client",
        material.client_static_bytes.as_ref(),
        &[client_body.as_slice(), transcript_hash],
    )?;
    append_len_prefixed(&mut client_init, &client_ed);
    pad_to_noise_block(&mut client_init);

    let mut relay_response = Vec::new();
    relay_response.push(HYBRID_RELAY_RESPONSE_TYPE);
    append_len_prefixed(&mut relay_response, params.relay_nonce);
    relay_response.extend_from_slice(&material.relay_ephemeral_public);
    append_len_prefixed(&mut relay_response, &material.relay_static_public);
    append_len_prefixed(&mut relay_response, params.relay_capabilities);
    append_len_prefixed(&mut relay_response, params.descriptor_commit);
    append_len_prefixed(&mut relay_response, &primary.relay_public);
    append_len_prefixed(&mut relay_response, &primary.ciphertext);
    append_len_prefixed(&mut relay_response, &primary.confirmation);
    append_len_prefixed(&mut relay_response, transcript_hash);
    let relay_body = relay_response.clone();
    let relay_dilithium = expand_material(
        b"soranet.sig.dilithium.hybrid.relay",
        &[
            material.relay_static_bytes.as_ref(),
            client_init.as_slice(),
            relay_body.as_slice(),
            transcript_hash,
        ],
        DILITHIUM3_SIGNATURE_LEN,
    );
    append_len_prefixed(&mut relay_response, &relay_dilithium);
    let relay_ed = derive_ed25519_signature(
        b"soranet.sig.ed25519.hybrid.relay",
        material.relay_static_bytes.as_ref(),
        &[
            client_init.as_slice(),
            relay_body.as_slice(),
            transcript_hash,
        ],
    )?;
    append_len_prefixed(&mut relay_response, &relay_ed);
    pad_to_noise_block(&mut relay_response);

    let mut telemetry_map = Map::new();
    telemetry_map.insert(
        "event".to_string(),
        Value::from("soranet_handshake_simulation"),
    );
    telemetry_map.insert(
        "handshake_suite".to_string(),
        Value::from(HandshakeSuite::Nk2Hybrid.label()),
    );
    telemetry_map.insert("kem_id".to_string(), Value::from(params.kem_id));
    telemetry_map.insert("sig_id".to_string(), Value::from(params.sig_id));
    telemetry_map.insert(
        "shared_secret_hex".to_string(),
        Value::from(hex::encode(&primary.shared_secret)),
    );
    telemetry_map.insert("warning_count".to_string(), Value::from(warnings.len()));
    telemetry_map.insert(
        "resume_hash_present".to_string(),
        Value::from(params.resume_hash.is_some()),
    );
    telemetry_map.insert(
        "transcript_hash_hex".to_string(),
        Value::from(hex::encode(transcript_hash)),
    );
    let payload_bytes = norito::json::to_vec(&Value::Object(telemetry_map.clone()))?;
    let (signature, witness) = signature_pair_from_static(
        material.relay_static_bytes.as_ref(),
        &payload_bytes,
        TELEMETRY_DILITHIUM_LABEL,
        TELEMETRY_ED25519_LABEL,
    )?;
    telemetry_map.insert("signature".to_string(), Value::from(signature));
    telemetry_map.insert("witness_signature".to_string(), Value::from(witness));
    let telemetry_json = norito::json::to_string_pretty(&Value::Object(telemetry_map))?;

    Ok(HandshakeArtifacts {
        steps: vec![
            HandshakeStep::new(
                "client",
                "HybridClientInit",
                STEP_NOTE_HYBRID_INIT,
                client_init,
            ),
            HandshakeStep::new(
                "relay",
                "HybridRelayResponse",
                STEP_NOTE_HYBRID_RESPONSE,
                relay_response,
            ),
        ],
        telemetry_payloads: vec![telemetry_json.into_bytes()],
    })
}

#[allow(clippy::too_many_lines)]
fn build_nk3_artifacts(
    params: &SimulationParams<'_>,
    transcript_hash: &[u8; 32],
    material: &DeterministicHandshakeMaterial,
    warnings: &[CapabilityWarning],
) -> Result<HandshakeArtifacts, HarnessError> {
    let primary = &material.primary_kem;
    let forward = &material.forward_secure_kem;
    let fs_commitment = Sha3_256::digest(forward.client_public.as_slice()).to_vec();
    let dual_mix = expand_material(
        b"soranet.kem.dual.mix",
        &[
            primary.shared_secret.as_slice(),
            forward.shared_secret.as_slice(),
            transcript_hash,
        ],
        forward.shared_secret.len(),
    );

    let mut client_commit = Vec::new();
    client_commit.push(PQFS_CLIENT_COMMIT_TYPE);
    append_len_prefixed(&mut client_commit, params.client_nonce);
    client_commit.push(u8::from(HandshakeSuite::Nk3PqForwardSecure));
    client_commit.push(params.kem_id);
    client_commit.push(params.sig_id);
    client_commit.extend_from_slice(&material.client_ephemeral_public);
    append_len_prefixed(&mut client_commit, &material.client_static_public);
    append_len_prefixed(&mut client_commit, &primary.client_public);
    append_len_prefixed(&mut client_commit, &forward.client_public);
    append_len_prefixed(&mut client_commit, params.client_capabilities);
    match params.resume_hash {
        Some(hash) => {
            client_commit.push(1);
            append_len_prefixed(&mut client_commit, hash);
        }
        None => client_commit.push(0),
    }
    append_len_prefixed(&mut client_commit, &fs_commitment);
    let client_body = client_commit.clone();
    let client_dilithium = expand_material(
        b"soranet.sig.dilithium.pqfs.client",
        &[
            material.client_static_bytes.as_ref(),
            client_body.as_slice(),
            transcript_hash,
        ],
        DILITHIUM3_SIGNATURE_LEN,
    );
    append_len_prefixed(&mut client_commit, &client_dilithium);
    let client_ed = derive_ed25519_signature(
        b"soranet.sig.ed25519.pqfs.client",
        material.client_static_bytes.as_ref(),
        &[client_body.as_slice(), transcript_hash],
    )?;
    append_len_prefixed(&mut client_commit, &client_ed);
    pad_to_noise_block(&mut client_commit);

    let mut relay_response = Vec::new();
    relay_response.push(PQFS_RELAY_RESPONSE_TYPE);
    append_len_prefixed(&mut relay_response, params.relay_nonce);
    relay_response.extend_from_slice(&material.relay_ephemeral_public);
    append_len_prefixed(&mut relay_response, &material.relay_static_public);
    append_len_prefixed(&mut relay_response, params.relay_capabilities);
    append_len_prefixed(&mut relay_response, params.descriptor_commit);
    append_len_prefixed(&mut relay_response, &primary.relay_public);
    append_len_prefixed(&mut relay_response, &primary.ciphertext);
    append_len_prefixed(&mut relay_response, &forward.relay_public);
    append_len_prefixed(&mut relay_response, &forward.ciphertext);
    append_len_prefixed(&mut relay_response, &primary.confirmation);
    append_len_prefixed(&mut relay_response, &forward.confirmation);
    append_len_prefixed(&mut relay_response, transcript_hash);
    append_len_prefixed(&mut relay_response, &fs_commitment);
    append_len_prefixed(&mut relay_response, &dual_mix);
    let relay_body = relay_response.clone();
    let relay_dilithium = expand_material(
        b"soranet.sig.dilithium.pqfs.relay",
        &[
            material.relay_static_bytes.as_ref(),
            client_commit.as_slice(),
            relay_body.as_slice(),
            transcript_hash,
        ],
        DILITHIUM3_SIGNATURE_LEN,
    );
    append_len_prefixed(&mut relay_response, &relay_dilithium);
    let relay_ed = derive_ed25519_signature(
        b"soranet.sig.ed25519.pqfs.relay",
        material.relay_static_bytes.as_ref(),
        &[
            client_commit.as_slice(),
            relay_body.as_slice(),
            transcript_hash,
        ],
    )?;
    append_len_prefixed(&mut relay_response, &relay_ed);
    pad_to_noise_block(&mut relay_response);

    let mut telemetry_map = Map::new();
    telemetry_map.insert(
        "event".to_string(),
        Value::from("soranet_handshake_simulation"),
    );
    telemetry_map.insert(
        "handshake_suite".to_string(),
        Value::from(HandshakeSuite::Nk3PqForwardSecure.label()),
    );
    telemetry_map.insert("kem_id".to_string(), Value::from(params.kem_id));
    telemetry_map.insert("sig_id".to_string(), Value::from(params.sig_id));
    telemetry_map.insert(
        "shared_secret_hex".to_string(),
        Value::from(hex::encode(&dual_mix)),
    );
    telemetry_map.insert(
        "fs_commitment_hex".to_string(),
        Value::from(hex::encode(&fs_commitment)),
    );
    telemetry_map.insert("warning_count".to_string(), Value::from(warnings.len()));
    telemetry_map.insert(
        "resume_hash_present".to_string(),
        Value::from(params.resume_hash.is_some()),
    );
    telemetry_map.insert(
        "transcript_hash_hex".to_string(),
        Value::from(hex::encode(transcript_hash)),
    );
    let payload_bytes = norito::json::to_vec(&Value::Object(telemetry_map.clone()))?;
    let (signature, witness) = signature_pair_from_static(
        material.relay_static_bytes.as_ref(),
        &payload_bytes,
        TELEMETRY_DILITHIUM_LABEL,
        TELEMETRY_ED25519_LABEL,
    )?;
    telemetry_map.insert("signature".to_string(), Value::from(signature));
    telemetry_map.insert("witness_signature".to_string(), Value::from(witness));
    let telemetry_json = norito::json::to_string_pretty(&Value::Object(telemetry_map))?;

    Ok(HandshakeArtifacts {
        steps: vec![
            HandshakeStep::new(
                "client",
                "PqfsClientCommit",
                STEP_NOTE_PQFS_COMMIT,
                client_commit,
            ),
            HandshakeStep::new(
                "relay",
                "PqfsRelayResponse",
                STEP_NOTE_PQFS_RESPONSE,
                relay_response,
            ),
        ],
        telemetry_payloads: vec![telemetry_json.into_bytes()],
    })
}

fn build_handshake_artifacts(
    params: &SimulationParams<'_>,
    client_static: &[u8; NOISE_SECRET_LEN],
    relay_static: &[u8; NOISE_SECRET_LEN],
    transcript_hash: &[u8; 32],
    suite: HandshakeSuite,
    warnings: &[CapabilityWarning],
) -> Result<HandshakeArtifacts, HarnessError> {
    let material = derive_handshake_material(params, client_static, relay_static)?;
    match suite {
        HandshakeSuite::Nk2Hybrid => {
            build_nk2_artifacts(params, transcript_hash, &material, warnings)
        }
        HandshakeSuite::Nk3PqForwardSecure => {
            build_nk3_artifacts(params, transcript_hash, &material, warnings)
        }
    }
}

fn validate_static_key(label: &str, key: &[u8]) -> Result<[u8; NOISE_SECRET_LEN], HarnessError> {
    if key.len() != NOISE_SECRET_LEN {
        return Err(HarnessError::Validation(format!(
            "{label} static key must be {NOISE_SECRET_LEN} bytes (X25519 secret), got {}",
            key.len()
        )));
    }
    let mut out = [0u8; NOISE_SECRET_LEN];
    out.copy_from_slice(key);
    Ok(out)
}

fn suite_for_kem_id(kem_id: u8) -> Result<MlKemSuite, HarnessError> {
    MlKemSuite::from_kem_id(kem_id).ok_or_else(|| {
        HarnessError::Validation(format!("unsupported ML-KEM identifier {kem_id:#04x}"))
    })
}

fn kem_profile(kem_id: u8) -> Result<KemProfile, HarnessError> {
    let suite = suite_for_kem_id(kem_id)?;
    let metadata = mlkem_metadata(suite);
    Ok(KemProfile { metadata })
}

fn derive_seed(label: &[u8], parts: &[&[u8]]) -> Result<[u8; NOISE_SECRET_LEN], HarnessError> {
    let material = expand_material(label, parts, NOISE_SECRET_LEN);
    material
        .try_into()
        .map_err(|_| HarnessError::Validation("failed to derive deterministic Noise secret".into()))
}

fn expand_material(label: &[u8], parts: &[&[u8]], len: usize) -> Vec<u8> {
    let mut hasher = Shake256::default();
    Update::update(&mut hasher, label);
    for part in parts {
        Update::update(&mut hasher, part);
    }
    let mut reader = hasher.finalize_xof();
    let mut out = vec![0u8; len];
    reader.read(&mut out);
    out
}

fn append_len_prefixed(buf: &mut Vec<u8>, data: &[u8]) {
    let len = u16::try_from(data.len()).expect("handshake field exceeds u16 length");
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(data);
}

fn pad_to_noise_block(buf: &mut Vec<u8>) {
    let rem = buf.len() % NOISE_PADDING_BLOCK;
    if rem != 0 {
        let pad_len = NOISE_PADDING_BLOCK - rem;
        buf.resize(buf.len() + pad_len, 0);
    }
}

fn derive_ed25519_signature(
    label: &[u8],
    static_key: &[u8],
    transcript_parts: &[&[u8]],
) -> Result<[u8; ED25519_SIGNATURE_LEN], HarnessError> {
    let seed = expand_material(label, &[static_key], NOISE_SECRET_LEN);
    let seed: [u8; NOISE_SECRET_LEN] = seed
        .try_into()
        .map_err(|_| HarnessError::Validation("failed to derive Ed25519 signing key".into()))?;
    let signing = SigningKey::from_bytes(&seed);

    let mut hasher = Sha3_256::new();
    Update::update(&mut hasher, STEP_DOMAIN);
    for part in transcript_parts {
        Update::update(&mut hasher, part);
    }
    let digest = hasher.finalize();
    let signature = signing.sign(&digest);
    Ok(signature.to_bytes())
}

/// Render a simulation report as pretty JSON.
///
/// # Errors
/// Returns an error if any warning or capability fails to serialize.
pub fn simulation_report_json(
    result: &SimulationResult,
    filter: Option<&[u16]>,
) -> Result<String, HarnessError> {
    use std::collections::BTreeSet;

    let filter_set = filter.map(|slice| slice.iter().copied().collect::<BTreeSet<_>>());
    let matches = |ty: u16| filter_set.as_ref().is_none_or(|set| set.contains(&ty));

    let filtered_warnings: Vec<&CapabilityWarning> = result
        .warnings
        .iter()
        .filter(|warn| matches(warn.capability_type))
        .collect();
    let filtered_client_caps: Vec<&CapabilityTlv> = result
        .client_capabilities
        .iter()
        .filter(|cap| matches(cap.ty))
        .collect();
    let filtered_relay_caps: Vec<&CapabilityTlv> = result
        .relay_capabilities
        .iter()
        .filter(|cap| matches(cap.ty))
        .collect();

    let capability_filter_value = filter_set.as_ref().map_or(Value::Null, |set| {
        let entries = set
            .iter()
            .map(|ty| Value::from(format!("0x{ty:04x}")))
            .collect::<Vec<_>>();
        Value::Array(entries)
    });

    let mut warnings_arr = Vec::new();
    for warning in filtered_warnings {
        warnings_arr.push(json::to_value(warning)?);
    }
    let warnings_value = Value::Array(warnings_arr);

    let mut client_caps_arr = Vec::new();
    for cap in filtered_client_caps {
        client_caps_arr.push(json::to_value(cap)?);
    }
    let client_caps_value = Value::Array(client_caps_arr);

    let mut relay_caps_arr = Vec::new();
    for cap in filtered_relay_caps {
        relay_caps_arr.push(json::to_value(cap)?);
    }
    let relay_caps_value = Value::Array(relay_caps_arr);

    let mut handshake_steps_arr = Vec::new();
    for step in &result.handshake_steps {
        handshake_steps_arr.push(json::to_value(step)?);
    }
    let handshake_steps_value = Value::Array(handshake_steps_arr);
    let telemetry_payloads_value = Value::Array(
        result
            .telemetry_payloads
            .iter()
            .map(|payload| Value::from(hex::encode(payload)))
            .collect(),
    );

    let mut map = Map::new();
    map.insert(
        "transcript_hash_hex".to_string(),
        Value::from(hex::encode(result.transcript_hash)),
    );
    map.insert(
        "handshake_suite".to_string(),
        Value::from(result.handshake_suite.label()),
    );
    map.insert("capability_filter_hex".to_string(), capability_filter_value);
    map.insert("warnings".to_string(), warnings_value);
    map.insert("client_capabilities".to_string(), client_caps_value);
    map.insert("relay_capabilities".to_string(), relay_caps_value);
    map.insert("handshake_steps".to_string(), handshake_steps_value);
    map.insert(
        "telemetry_payloads_hex".to_string(),
        telemetry_payloads_value,
    );

    Ok(json::to_string_pretty(&Value::Object(map))?)
}

#[derive(Clone, Copy)]
struct FixtureSpec {
    id: &'static str,
    description: &'static str,
    client_hex: &'static str,
    relay_hex: &'static str,
    descriptor_commit_hex: &'static str,
    client_nonce_hex: &'static str,
    relay_nonce_hex: &'static str,
    resume_hash_hex: Option<&'static str>,
    kem_id: u8,
    sig_id: u8,
    transcript_hash_hex: &'static str,
    warnings: &'static [&'static str],
    expected_outcome: &'static str,
    expected_alarm: Option<AlarmSpec>,
    relay_role: u8,
}

#[derive(Clone, Copy)]
struct AlarmSpec {
    relay_id: &'static str,
    capability_type: u16,
    reason: &'static str,
    capability_value_hex: Option<&'static str>,
}

const FIXTURES: &[FixtureSpec] = &[
    FixtureSpec {
        id: "snnet-cap-001-success",
        description: "PQ-capable guard echoes ML-KEM-768 + Dilithium3",
        client_hex: "0101000201010102000201010104000282030202000200047f100004deadbeef7f110004cafebabe",
        relay_hex: "0101000201010102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f01040002820302010001010202000200047f12000412345678",
        descriptor_commit_hex: "76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f",
        client_nonce_hex: "2c1f64028dbe42410d1921cd9a316bed4f8f5b52ffb62b4dcaf149048393ca8a",
        relay_nonce_hex: "d5f4f2f9c2b1a39e88bbd3c0a4f9e178d93e7bfacaf0c3e872b712f4a341c9de",
        resume_hash_hex: None,
        kem_id: 1,
        sig_id: 1,
        transcript_hash_hex: "fd92a86d953dfa161d27c79785d495108bfd6991d3ed3baff11752baff4e0bf8",
        warnings: &[],
        expected_outcome: "success",
        expected_alarm: None,
        relay_role: 0x01,
    },
    FixtureSpec {
        id: "snnet-cap-002-downgrade",
        description: "Relay strips snnet.pqkem; client aborts with downgrade alarm",
        client_hex: "0101000201010102000201010104000282030202000200047f100004deadbeef7f110004cafebabe",
        relay_hex: "0102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f01040002820302010001010202000200047f1300040badc0de",
        descriptor_commit_hex: "76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f",
        client_nonce_hex: "1f2e3d4c5b6a79888796a5b4c3d2e1f00112233445566778899aabbccddeeff0",
        relay_nonce_hex: "2b64a7e5c1d3f4b2a9c8d7e6f5a4132233445566778899aabbccddeeff001122",
        resume_hash_hex: None,
        kem_id: 1,
        sig_id: 1,
        transcript_hash_hex: "8ff199b86cbc6a80b38321fbe8f6723ea705141f12bdbc940d573cce39379839",
        warnings: &["relay missing required snnet.pqkem"],
        expected_outcome: "downgrade_abort",
        expected_alarm: Some(AlarmSpec {
            relay_id: "8N3j9SJgQnJhq4nsuMUSg31d2PmH3xXucn71bXQyvYy4",
            capability_type: 0x0101,
            reason: "MissingRequiredKEM",
            capability_value_hex: None,
        }),
        relay_role: 0x01,
    },
    FixtureSpec {
        id: "snnet-cap-006-constant-rate",
        description: "Client requires snnet.constant_rate; relay lacking TLV triggers downgrade",
        client_hex: "0101000201010102000201010104000282030202000200047f100004deadbeef7f110004cafebabe020300020101",
        relay_hex: "0101000201010102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f01040002820302010001010202000200047f12000412345678",
        descriptor_commit_hex: "76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f",
        client_nonce_hex: "1f2e3d4c5b6a79888796a5b4c3d2e1f00112233445566778899aabbccddeeff0",
        relay_nonce_hex: "2b64a7e5c1d3f4b2a9c8d7e6f5a4132233445566778899aabbccddeeff001122",
        resume_hash_hex: None,
        kem_id: 1,
        sig_id: 1,
        transcript_hash_hex: "78e0c7782b4cfa1a6c0ffe750562776b986be13514ff7b7b42ac5a17936ff680",
        warnings: &["relay missing required capability type=0x0203 (snnet.constant_rate)"],
        expected_outcome: "downgrade_abort",
        expected_alarm: None,
        relay_role: 0x01,
    },
    FixtureSpec {
        id: "snnet-cap-003-digest-mismatch",
        description: "Relay echoes PQ TLVs but mutates the transcript commit digest",
        client_hex: "0101000201010102000201010104000282030202000200047f100004deadbeef7f110004cafebabe",
        relay_hex: "01010002010101020002010101030020ea157a5af59deee040cfc371a724790ed969d32295e05fea6a4ff4539449827501040002820302010001010202000200047f140004cafed00d",
        descriptor_commit_hex: "ea157a5af59deee040cfc371a724790ed969d32295e05fea6a4ff45394498275",
        client_nonce_hex: "abcdef0123456789fedcba98765432100123456789abcdef0011223344556677",
        relay_nonce_hex: "00112233445566778899aabbccddeeff112233445566778899aabbccddeeff11",
        resume_hash_hex: None,
        kem_id: 1,
        sig_id: 1,
        transcript_hash_hex: "a5d11071d76f3f367d1c03dcd9fc95200faf7e4c9d69c06f99a5ac2ed4b0917a",
        warnings: &["relay transcript commit differs"],
        expected_outcome: "digest_mismatch",
        expected_alarm: None,
        relay_role: 0x01,
    },
    FixtureSpec {
        id: "snnet-cap-004-grease",
        description: "Negotiation succeeds while preserving GREASE TLVs for transcript hashing",
        client_hex: "0101000201010102000201010104000282030202000280047f200004112233447f2100085566778899aabbcc",
        relay_hex: "0101000201010102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f01040002820302010001010202000200047f220004deadc0de7f2300080011223344556677",
        descriptor_commit_hex: "76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f",
        client_nonce_hex: "112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00",
        relay_nonce_hex: "cafebabefeedface00112233445566778899aabbccddeeff0011223344556677",
        resume_hash_hex: Some("aabbccddeeff00112233445566778899"),
        kem_id: 1,
        sig_id: 1,
        transcript_hash_hex: "3c2d5722530ebee2557851e6626033a9c90edb8507527d24f163019d98ff3ed8",
        warnings: &[],
        expected_outcome: "success",
        expected_alarm: None,
        relay_role: 0x01,
    },
];

struct InteropSpec {
    id: &'static str,
    description: &'static str,
    suite: HandshakeSuite,
    client_suites: &'static [HandshakeSuite],
    client_required: bool,
    relay_suites: &'static [HandshakeSuite],
    relay_required: bool,
    client_static_sk_hex: &'static str,
    relay_static_sk_hex: &'static str,
    client_nonce_hex: &'static str,
    relay_nonce_hex: &'static str,
    resume_hash_hex: Option<&'static str>,
    kem_id: u8,
    sig_id: u8,
}

const INTEROP_LANGUAGES: &[&str] = &["rust", "go", "cpp"];

const INTEROP_SPECS: &[InteropSpec] = &[
    InteropSpec {
        id: "snnet-interop-nk2-v1",
        description: "NK2 hybrid handshake baseline vector",
        suite: HandshakeSuite::Nk2Hybrid,
        client_suites: &[
            HandshakeSuite::Nk2Hybrid,
            HandshakeSuite::Nk3PqForwardSecure,
        ],
        client_required: true,
        relay_suites: &[
            HandshakeSuite::Nk2Hybrid,
            HandshakeSuite::Nk3PqForwardSecure,
        ],
        relay_required: true,
        client_static_sk_hex: "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff",
        relay_static_sk_hex: "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100",
        client_nonce_hex: "101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f",
        relay_nonce_hex: "303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f",
        resume_hash_hex: None,
        kem_id: 1,
        sig_id: 1,
    },
    InteropSpec {
        id: "snnet-interop-nk3-v1",
        description: "NK3 forward-secure handshake baseline vector",
        suite: HandshakeSuite::Nk3PqForwardSecure,
        client_suites: &[
            HandshakeSuite::Nk3PqForwardSecure,
            HandshakeSuite::Nk2Hybrid,
        ],
        client_required: true,
        relay_suites: &[
            HandshakeSuite::Nk3PqForwardSecure,
            HandshakeSuite::Nk2Hybrid,
        ],
        relay_required: true,
        client_static_sk_hex: "c0c1c2c3c4c5c6c7c8c9cacbcccdcecf404142434445464748494a4b4c4d4e4f",
        relay_static_sk_hex: "505152535455565758595a5b5c5d5e5f606162636465666768696a6b6c6d6e6f",
        client_nonce_hex: "00f1e2d3c4b5a69788796a5b4c3d2e1f00112233445566778899aabbccddeeff",
        relay_nonce_hex: "0f0e1d2c3b4a5968778695a4b3c2d1e0ff1e2d3c4b5a69788796a5b4c3d2e1f0",
        resume_hash_hex: None,
        kem_id: 1,
        sig_id: 1,
    },
];

struct SaltFixtureSpec {
    file: &'static str,
    epoch_id: u32,
    previous_epoch: Option<u32>,
    valid_after: &'static str,
    valid_until: &'static str,
    blinded_cid_salt_hex: &'static str,
    emergency_rotation: bool,
    notes: Option<&'static str>,
}

const SALT_FIXTURES: &[SaltFixtureSpec] = &[SaltFixtureSpec {
    file: "epoch-000042.norito.json",
    epoch_id: 42,
    previous_epoch: Some(41),
    valid_after: "2026-01-01T00:00:00Z",
    valid_until: "2026-01-02T00:00:00Z",
    blinded_cid_salt_hex: "ee4fd5a8a2b4e9c0f1dc1a67c962e8d9b4a1c0ffee112233445566778899aabb",
    emergency_rotation: false,
    notes: Some("Routine rotation"),
}];

/// Generate JSON fixtures describing supported capability combinations.
///
/// # Errors
/// Returns an error if fixture files cannot be written to the target directory.
pub fn generate_capability_fixtures(output: &Path) -> Result<(), HarnessError> {
    fs::create_dir_all(output)?;
    for spec in FIXTURES {
        write_fixture(output, spec)?;
        if let Some(alarm) = spec.expected_alarm {
            write_alarm_fixture(output, spec, alarm)?;
        }
    }
    let salt_dir = salt_dir(output);
    generate_salt_fixtures(&salt_dir)?;
    generate_interop_vectors(output)?;
    Ok(())
}

/// Verify existing capability fixtures by re-running the simulations and alarms.
///
/// # Errors
/// Returns an error when fixture contents fail validation or cannot be read.
pub fn verify_fixtures(output: &Path) -> Result<(), HarnessError> {
    let canonical = canonical_path(output);
    if !canonical.exists() {
        return Err(HarnessError::Validation(format!(
            "expected fixture directory {path} to exist; run `cargo xtask soranet-fixtures` first",
            path = canonical.display()
        )));
    }

    let temp = TempDir::new()?;
    let temp_capabilities = temp.path().join("capabilities");
    fs::create_dir_all(&temp_capabilities)?;
    generate_capability_fixtures(&temp_capabilities)?;

    for spec in FIXTURES {
        compare_fixture(
            &capability_path(&temp_capabilities, spec.id),
            &capability_path(&canonical, spec.id),
        )?;
        if spec.expected_alarm.is_some() {
            compare_fixture(
                &telemetry_path(&temp_capabilities, spec.id),
                &telemetry_path(&canonical, spec.id),
            )?;
        }
    }

    let canonical_salt = salt_dir(&canonical);
    let temp_salt = salt_dir(&temp_capabilities);
    for spec in SALT_FIXTURES {
        compare_fixture(
            &salt_path(&temp_salt, spec.file),
            &salt_path(&canonical_salt, spec.file),
        )?;
    }

    compare_interop_vectors(&temp_capabilities, &canonical)?;

    println!("verified fixtures at {}", canonical.display());
    Ok(())
}

fn interop_dir(base: &Path) -> PathBuf {
    base.parent()
        .map_or_else(|| base.join("interop"), |parent| parent.join("interop"))
}

fn interop_language_dir(base: &Path, language: &str) -> PathBuf {
    interop_dir(base).join(language)
}

fn interop_vector_path(base: &Path, language: &str, spec: &InteropSpec) -> PathBuf {
    interop_language_dir(base, language).join(format!("{}.json", spec.id))
}

fn decode_fixed_hex<const N: usize>(label: &str, input: &str) -> Result<[u8; N], HarnessError> {
    let bytes = decode_hex(input)?;
    if bytes.len() != N {
        return Err(HarnessError::Validation(format!(
            "{label} must decode to {N} bytes, got {}",
            bytes.len()
        )));
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes);
    Ok(out)
}

struct CapabilityContext {
    client_caps: Vec<u8>,
    relay_caps: Vec<u8>,
    client_tlvs: Vec<CapabilityTlv>,
    relay_tlvs: Vec<CapabilityTlv>,
    warnings: Vec<CapabilityWarning>,
}

fn prepare_capability_context(spec: &InteropSpec) -> Result<CapabilityContext, HarnessError> {
    let client_caps = update_suite_list(
        &DEFAULT_CLIENT_CAPABILITIES,
        spec.client_suites,
        spec.client_required,
    )?;
    let relay_caps = update_suite_list(
        &DEFAULT_RELAY_CAPABILITIES,
        spec.relay_suites,
        spec.relay_required,
    )?;
    let client_tlvs = parse_capabilities(&client_caps)?;
    let relay_tlvs = parse_capabilities(&relay_caps)?;

    let HandshakeSuiteNegotiation {
        selected,
        mut warnings,
    } = negotiate_handshake_suite(&client_tlvs, &relay_tlvs)?;
    if selected != spec.suite {
        return Err(HarnessError::Validation(format!(
            "interop spec {} expected {}, negotiated {}",
            spec.id,
            spec.suite.label(),
            selected.label()
        )));
    }
    warnings.extend(diff_capabilities(&client_tlvs, &relay_tlvs));

    Ok(CapabilityContext {
        client_caps,
        relay_caps,
        client_tlvs,
        relay_tlvs,
        warnings,
    })
}

struct HandshakeInputs {
    client_static: [u8; NOISE_SECRET_LEN],
    relay_static: [u8; NOISE_SECRET_LEN],
    client_nonce: [u8; NOISE_SECRET_LEN],
    relay_nonce: [u8; NOISE_SECRET_LEN],
    resume_hash: Option<Vec<u8>>,
}

fn decode_handshake_inputs(spec: &InteropSpec) -> Result<HandshakeInputs, HarnessError> {
    Ok(HandshakeInputs {
        client_static: decode_fixed_hex("client_static_sk_hex", spec.client_static_sk_hex)?,
        relay_static: decode_fixed_hex("relay_static_sk_hex", spec.relay_static_sk_hex)?,
        client_nonce: decode_fixed_hex("client_nonce_hex", spec.client_nonce_hex)?,
        relay_nonce: decode_fixed_hex("relay_nonce_hex", spec.relay_nonce_hex)?,
        resume_hash: spec.resume_hash_hex.map(decode_hex).transpose()?,
    })
}

fn build_simulation_params<'a>(
    spec: &InteropSpec,
    capabilities: &'a CapabilityContext,
    inputs: &'a HandshakeInputs,
) -> SimulationParams<'a> {
    SimulationParams {
        client_capabilities: &capabilities.client_caps,
        relay_capabilities: &capabilities.relay_caps,
        client_static_sk: &inputs.client_static,
        relay_static_sk: &inputs.relay_static,
        resume_hash: inputs.resume_hash.as_deref(),
        descriptor_commit: &DEFAULT_DESCRIPTOR_COMMIT,
        client_nonce: &inputs.client_nonce,
        relay_nonce: &inputs.relay_nonce,
        kem_id: spec.kem_id,
        sig_id: spec.sig_id,
    }
}

fn compute_transcript_hash(params: &SimulationParams<'_>, suite: HandshakeSuite) -> [u8; 32] {
    TranscriptInputs {
        descriptor_commit: params.descriptor_commit,
        client_nonce: params.client_nonce,
        relay_nonce: params.relay_nonce,
        capability_bytes: params.client_capabilities,
        kem_id: params.kem_id,
        sig_id: params.sig_id,
        handshake_suite: suite,
        resume_hash: params.resume_hash,
    }
    .compute_hash()
}

struct SessionArtifacts {
    handshake: HandshakeArtifacts,
    session_key: Vec<u8>,
    session_confirmation: Vec<u8>,
    primary_shared: Vec<u8>,
    forward_shared: Option<Vec<u8>>,
    dual_mix: Option<Vec<u8>>,
}

fn build_session_artifacts(
    spec: &InteropSpec,
    params: &SimulationParams<'_>,
    transcript_hash: &[u8; 32],
    material: &DeterministicHandshakeMaterial,
    warnings: &[CapabilityWarning],
) -> Result<SessionArtifacts, HarnessError> {
    let handshake = match spec.suite {
        HandshakeSuite::Nk2Hybrid => {
            build_nk2_artifacts(params, transcript_hash, material, warnings)?
        }
        HandshakeSuite::Nk3PqForwardSecure => {
            build_nk3_artifacts(params, transcript_hash, material, warnings)?
        }
    };

    let forward_shared = if spec.suite == HandshakeSuite::Nk3PqForwardSecure {
        Some(material.forward_secure_kem.shared_secret.clone())
    } else {
        None
    };

    let dual_mix = if spec.suite == HandshakeSuite::Nk3PqForwardSecure {
        Some(expand_material(
            b"soranet.kem.dual.mix",
            &[
                material.primary_kem.shared_secret.as_slice(),
                material.forward_secure_kem.shared_secret.as_slice(),
                transcript_hash,
            ],
            material.forward_secure_kem.shared_secret.len(),
        ))
    } else {
        None
    };

    let session_inputs = SessionKeyInputs {
        suite: spec.suite,
        transcript_hash,
        primary_shared: material.primary_kem.shared_secret.as_slice(),
        forward_shared: forward_shared.as_deref(),
    };
    let (session_key, nk2_confirmation) = derive_session_key_and_confirmation(session_inputs)?;

    Ok(SessionArtifacts {
        handshake,
        session_key,
        session_confirmation: nk2_confirmation,
        primary_shared: material.primary_kem.shared_secret.clone(),
        forward_shared,
        dual_mix,
    })
}

fn telemetry_payload_values(artifacts: &HandshakeArtifacts) -> Result<Vec<Value>, HarnessError> {
    let mut telemetry_arr = Vec::new();
    for payload in &artifacts.telemetry_payloads {
        let text = std::str::from_utf8(payload)
            .map_err(|_| HarnessError::Validation("telemetry payload is not valid UTF-8".into()))?;
        telemetry_arr.push(Value::from(text.to_owned()));
    }
    Ok(telemetry_arr)
}

fn warning_values(warnings: &[CapabilityWarning]) -> Result<Vec<Value>, HarnessError> {
    Ok(warnings
        .iter()
        .map(json::to_value)
        .collect::<Result<Vec<_>, _>>()?)
}

fn handshake_step_values(artifacts: &HandshakeArtifacts) -> Result<Vec<Value>, HarnessError> {
    Ok(artifacts
        .steps
        .iter()
        .map(json::to_value)
        .collect::<Result<Vec<_>, _>>()?)
}

fn optional_hex(bytes: Option<&[u8]>) -> Value {
    bytes.map_or(Value::Null, |slice| Value::from(hex::encode(slice)))
}

fn render_inputs_map(
    spec: &InteropSpec,
    capabilities: &CapabilityContext,
    resume_bytes: Option<&Vec<u8>>,
) -> Result<Map, HarnessError> {
    let client_caps_value = capabilities
        .client_tlvs
        .iter()
        .map(json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    let relay_caps_value = capabilities
        .relay_tlvs
        .iter()
        .map(json::to_value)
        .collect::<Result<Vec<_>, _>>()?;

    let mut inputs_map = Map::new();
    inputs_map.insert(
        "client_capabilities_hex".to_string(),
        Value::from(hex::encode(&capabilities.client_caps)),
    );
    inputs_map.insert(
        "relay_capabilities_hex".to_string(),
        Value::from(hex::encode(&capabilities.relay_caps)),
    );
    inputs_map.insert(
        "client_capabilities".to_string(),
        Value::from(client_caps_value),
    );
    inputs_map.insert(
        "relay_capabilities".to_string(),
        Value::from(relay_caps_value),
    );
    inputs_map.insert(
        "client_static_sk_hex".to_string(),
        Value::from(spec.client_static_sk_hex),
    );
    inputs_map.insert(
        "relay_static_sk_hex".to_string(),
        Value::from(spec.relay_static_sk_hex),
    );
    inputs_map.insert(
        "client_nonce_hex".to_string(),
        Value::from(spec.client_nonce_hex),
    );
    inputs_map.insert(
        "relay_nonce_hex".to_string(),
        Value::from(spec.relay_nonce_hex),
    );
    inputs_map.insert(
        "descriptor_commit_hex".to_string(),
        Value::from(hex::encode(DEFAULT_DESCRIPTOR_COMMIT)),
    );
    inputs_map.insert(
        "resume_hash_hex".to_string(),
        resume_bytes.map_or(Value::Null, |bytes| Value::from(hex::encode(bytes))),
    );
    Ok(inputs_map)
}

fn render_outputs_map(
    spec: &InteropSpec,
    transcript_hash: &[u8; 32],
    material: &DeterministicHandshakeMaterial,
    session: &SessionArtifacts,
    warnings: &[CapabilityWarning],
) -> Result<Map, HarnessError> {
    let steps_arr = handshake_step_values(&session.handshake)?;
    let telemetry_arr = telemetry_payload_values(&session.handshake)?;
    let warnings_arr = warning_values(warnings)?;

    let mut outputs_map = Map::new();
    outputs_map.insert(
        "transcript_hash_hex".to_string(),
        Value::from(hex::encode(transcript_hash)),
    );
    outputs_map.insert(
        "session_key_hex".to_string(),
        Value::from(hex::encode(&session.session_key)),
    );
    outputs_map.insert(
        "primary_shared_secret_hex".to_string(),
        Value::from(hex::encode(&session.primary_shared)),
    );
    outputs_map.insert(
        "primary_confirmation_hex".to_string(),
        Value::from(hex::encode(&material.primary_kem.confirmation)),
    );
    outputs_map.insert(
        "session_confirmation_hex".to_string(),
        Value::from(hex::encode(&session.session_confirmation)),
    );
    outputs_map.insert(
        "forward_shared_secret_hex".to_string(),
        optional_hex(session.forward_shared.as_deref()),
    );
    outputs_map.insert(
        "forward_confirmation_hex".to_string(),
        if spec.suite == HandshakeSuite::Nk3PqForwardSecure {
            Value::from(hex::encode(&material.forward_secure_kem.confirmation))
        } else {
            Value::Null
        },
    );
    outputs_map.insert(
        "dual_mix_hex".to_string(),
        optional_hex(session.dual_mix.as_deref()),
    );
    outputs_map.insert("handshake_steps".to_string(), Value::from(steps_arr));
    outputs_map.insert("telemetry_payloads".to_string(), Value::from(telemetry_arr));
    outputs_map.insert("warnings".to_string(), Value::from(warnings_arr));
    Ok(outputs_map)
}

fn build_interop_value(spec: &InteropSpec) -> Result<Value, HarnessError> {
    let capability_context = prepare_capability_context(spec)?;
    let inputs = decode_handshake_inputs(spec)?;
    let params = build_simulation_params(spec, &capability_context, &inputs);
    let transcript_hash = compute_transcript_hash(&params, spec.suite);

    let material = derive_handshake_material(&params, &inputs.client_static, &inputs.relay_static)?;
    let session = build_session_artifacts(
        spec,
        &params,
        &transcript_hash,
        &material,
        &capability_context.warnings,
    )?;

    let inputs_map = render_inputs_map(spec, &capability_context, inputs.resume_hash.as_ref())?;
    let outputs_map = render_outputs_map(
        spec,
        &transcript_hash,
        &material,
        &session,
        &capability_context.warnings,
    )?;

    let mut root = Map::new();
    root.insert("id".to_string(), Value::from(spec.id));
    root.insert("description".to_string(), Value::from(spec.description));
    root.insert("suite".to_string(), Value::from(spec.suite.label()));
    root.insert(
        "suite_id_hex".to_string(),
        Value::from(format!("0x{:02x}", u8::from(spec.suite))),
    );
    root.insert("kem_id".to_string(), Value::from(spec.kem_id));
    root.insert("sig_id".to_string(), Value::from(spec.sig_id));
    root.insert("inputs".to_string(), Value::Object(inputs_map));
    root.insert("outputs".to_string(), Value::Object(outputs_map));

    Ok(Value::Object(root))
}

fn generate_interop_vectors(base: &Path) -> Result<(), HarnessError> {
    for language in INTEROP_LANGUAGES {
        fs::create_dir_all(interop_language_dir(base, language))?;
    }
    for spec in INTEROP_SPECS {
        let base_value = build_interop_value(spec)?;
        for language in INTEROP_LANGUAGES {
            let mut value = base_value.clone();
            if let Value::Object(ref mut map) = value {
                map.insert("language".to_string(), Value::from(*language));
            }
            let rendered = norito::json::to_string_pretty(&value)?;
            let path = interop_vector_path(base, language, spec);
            fs::write(path, rendered)?;
        }
    }
    Ok(())
}

fn compare_interop_vectors(temp: &Path, canonical: &Path) -> Result<(), HarnessError> {
    for spec in INTEROP_SPECS {
        for language in INTEROP_LANGUAGES {
            compare_fixture(
                &interop_vector_path(temp, language, spec),
                &interop_vector_path(canonical, language, spec),
            )?;
        }
    }
    Ok(())
}

fn write_fixture(output: &Path, spec: &FixtureSpec) -> Result<(), HarnessError> {
    decode_hex(spec.client_hex)?;
    decode_hex(spec.relay_hex)?;
    decode_hex(spec.descriptor_commit_hex)?;
    decode_hex(spec.client_nonce_hex)?;
    decode_hex(spec.relay_nonce_hex)?;
    if let Some(hex) = spec.resume_hash_hex {
        decode_hex(hex)?;
    }

    let warnings_values = spec
        .warnings
        .iter()
        .map(|warning| Value::from(*warning))
        .collect::<Vec<_>>();

    let expected_alarm_value = spec
        .expected_alarm
        .map(|alarm| signed_alarm_value(spec, alarm, &FIXTURE_RELAY_SIGNATURE_KEY))
        .transpose()?;

    let mut map = Map::new();
    map.insert("fixture_id".to_string(), Value::from(spec.id));
    map.insert("description".to_string(), Value::from(spec.description));
    map.insert(
        "client_vector_hex".to_string(),
        Value::from(spec.client_hex),
    );
    map.insert("relay_vector_hex".to_string(), Value::from(spec.relay_hex));
    map.insert(
        "client_nonce_hex".to_string(),
        Value::from(spec.client_nonce_hex),
    );
    map.insert(
        "relay_nonce_hex".to_string(),
        Value::from(spec.relay_nonce_hex),
    );
    map.insert(
        "descriptor_commit_hex".to_string(),
        Value::from(spec.descriptor_commit_hex),
    );
    map.insert(
        "resume_hash_hex".to_string(),
        spec.resume_hash_hex.map_or(Value::Null, Value::from),
    );
    map.insert("kem_id".to_string(), Value::from(spec.kem_id));
    map.insert("sig_id".to_string(), Value::from(spec.sig_id));
    map.insert(
        "transcript_hash_hex".to_string(),
        Value::from(spec.transcript_hash_hex),
    );
    map.insert("warnings".to_string(), Value::Array(warnings_values));
    map.insert(
        "expected_outcome".to_string(),
        Value::from(spec.expected_outcome),
    );
    map.insert(
        "expected_alarm".to_string(),
        expected_alarm_value.unwrap_or(Value::Null),
    );
    map.insert("fixture_signature".to_string(), Value::Null);
    let fixture_json = Value::Object(map);

    let path = capability_path(output, spec.id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut json_text = norito::json::to_string_pretty(&fixture_json)?;
    json_text.push('\n');
    fs::write(&path, json_text.as_bytes())?;
    println!("wrote {}", canonical_path(&path).display());
    Ok(())
}

fn write_alarm_fixture(
    output: &Path,
    spec: &FixtureSpec,
    alarm: AlarmSpec,
) -> Result<(), HarnessError> {
    let mut map = Map::new();
    map.insert("relay_id".to_string(), Value::from(alarm.relay_id));
    map.insert("relay_role".to_string(), Value::from(spec.relay_role));
    map.insert("client_id".to_string(), Value::Null);
    map.insert(
        "capability_type".to_string(),
        Value::from(alarm.capability_type),
    );
    map.insert(
        "capability_value_hex".to_string(),
        alarm.capability_value_hex.map_or(Value::Null, Value::from),
    );
    map.insert("reason_code".to_string(), Value::from(alarm.reason));
    map.insert(
        "transcript_hash_hex".to_string(),
        Value::from(spec.transcript_hash_hex),
    );
    map.insert(
        "observed_at_unix_ms".to_string(),
        Value::from(1_785_120_000_000u64),
    );
    map.insert("circuit_id".to_string(), Value::from(20_882u64));
    map.insert("counter".to_string(), Value::from(1u32));
    map.insert("signature".to_string(), Value::Null);
    let json = Value::Object(map);

    let path = telemetry_path(output, spec.id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, norito::json::to_string_pretty(&json)? + "\n")?;
    println!("wrote {}", canonical_path(&path).display());
    Ok(())
}

fn generate_salt_fixtures(dir: &Path) -> Result<(), HarnessError> {
    fs::create_dir_all(dir)?;
    for spec in SALT_FIXTURES {
        write_salt_fixture(dir, spec)?;
    }
    Ok(())
}

fn write_salt_fixture(dir: &Path, spec: &SaltFixtureSpec) -> Result<(), HarnessError> {
    let salt = decode_salt_hex(spec.blinded_cid_salt_hex)?;
    let json = salt_announcement_json(&SaltAnnouncementParams {
        epoch_id: spec.epoch_id,
        previous_epoch: spec.previous_epoch,
        valid_after: spec.valid_after,
        valid_until: spec.valid_until,
        blinded_cid_salt: &salt,
        emergency_rotation: spec.emergency_rotation,
        notes: spec.notes,
        signature: None,
    })?;

    let path = salt_path(dir, spec.file);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, json + "\n")?;
    println!("wrote {}", canonical_path(&path).display());
    Ok(())
}

fn capability_path(root: &Path, spec_id: &str) -> PathBuf {
    root.join(format!("{spec_id}.norito.json"))
}

fn telemetry_path(root: &Path, spec_id: &str) -> PathBuf {
    root.parent()
        .unwrap_or(root)
        .join("telemetry")
        .join(format!("{spec_id}-alarm.norito.json"))
}

fn salt_dir(capabilities_dir: &Path) -> PathBuf {
    capabilities_dir
        .parent()
        .unwrap_or(capabilities_dir)
        .join("salt")
}

fn salt_path(root: &Path, file: &str) -> PathBuf {
    root.join(file)
}

fn canonical_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn compare_fixture(expected: &Path, actual: &Path) -> Result<(), HarnessError> {
    if !actual.exists() {
        return Err(HarnessError::Validation(format!(
            "expected fixture {} to exist; run `cargo xtask soranet-fixtures` to regenerate",
            actual.display()
        )));
    }
    let expected_bytes = fs::read(expected)?;
    let actual_bytes = fs::read(actual)?;
    if expected_bytes != actual_bytes {
        return Err(HarnessError::Validation(format!(
            "fixture {} is out of date; run `cargo xtask soranet-fixtures` to refresh",
            actual.display()
        )));
    }
    Ok(())
}

/// Descriptor commitment used in the reference fixture bundle.
pub const DEFAULT_DESCRIPTOR_COMMIT: [u8; 32] =
    hex_literal::hex!("76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f");

/// Default capability vector advertised by clients during the handshake.
pub const DEFAULT_CLIENT_CAPABILITIES: [u8; 40] = hex_literal::hex!(
    "0101000201010102000201010104000282030202000200047f100004deadbeef7f110004cafebabe"
);

/// Default capability vector advertised by relays during the handshake.
pub const DEFAULT_RELAY_CAPABILITIES: [u8; 73] = hex_literal::hex!(
    "0101000201010102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f01040002820302010001010202000200047f12000412345678"
);

/// Parameters controlling the runtime handshake.
#[derive(Clone)]
pub struct RuntimeParams<'a> {
    /// Commitment to the relay descriptor that anchors the transcript hash.
    pub descriptor_commit: &'a [u8],
    /// Capability TLVs advertised by the client.
    pub client_capabilities: &'a [u8],
    /// Capability TLVs advertised by the relay.
    pub relay_capabilities: &'a [u8],
    /// ML-KEM identifier negotiated for the session.
    pub kem_id: u8,
    /// Signature suite identifier negotiated for the session.
    pub sig_id: u8,
    /// Optional resume hash carried over from a previous circuit.
    pub resume_hash: Option<&'a [u8]>,
}

impl RuntimeParams<'_> {
    /// Return the canonical runtime parameters matching the published fixtures.
    #[must_use]
    pub fn soranet_defaults() -> Self {
        Self {
            descriptor_commit: &DEFAULT_DESCRIPTOR_COMMIT,
            client_capabilities: &DEFAULT_CLIENT_CAPABILITIES,
            relay_capabilities: &DEFAULT_RELAY_CAPABILITIES,
            kem_id: 1,
            sig_id: 1,
            resume_hash: None,
        }
    }
}

/// Shared handshake outcome containing the derived session key and metadata.
pub struct SessionSecrets {
    /// Session key derived from the hybrid handshake (Kyber + X25519 + HKDF).
    pub session_key: Vec<u8>,
    /// Transcript hash binding descriptor, nonces, and capability TLVs.
    pub transcript_hash: [u8; 32],
    /// Negotiated handshake suite for the runtime session.
    pub handshake_suite: HandshakeSuite,
    /// Capability warnings raised during negotiation.
    pub warnings: Vec<CapabilityWarning>,
    /// Optional telemetry payload emitted as part of the handshake.
    pub telemetry_payload: Option<Vec<u8>>,
}

/// Opaque state retained by the client between `ClientHello` and `ClientFinish`.
#[allow(dead_code)]
pub struct ClientState {
    client_nonce: [u8; 32],
    client_ephemeral_secret: StaticSecret,
    client_ephemeral_public: [u8; 32],
    client_static_secret: StaticSecret,
    client_static_public: [u8; 32],
    client_kem_secret: Zeroizing<Vec<u8>>,
    client_kem_public: Vec<u8>,
    forward_kem_secret: Option<Zeroizing<Vec<u8>>>,
    forward_kem_public: Option<Vec<u8>>,
    client_capabilities: Vec<u8>,
    descriptor_commit: Vec<u8>,
    resume_hash: Option<Vec<u8>>,
    kem_id: u8,
    sig_id: u8,
    handshake_suite: HandshakeSuite,
    client_hello: Vec<u8>,
}

/// Opaque state retained by the relay between `RelayHello` and `ClientFinish`.
#[allow(dead_code)]
pub struct RelayState {
    client_nonce: [u8; 32],
    client_ephemeral_public: [u8; 32],
    client_capabilities: Vec<u8>,
    client_kem_public: Vec<u8>,
    resume_hash: Option<Vec<u8>>,
    client_static_public: Option<[u8; 32]>,
    relay_nonce: [u8; 32],
    relay_ephemeral_secret: StaticSecret,
    relay_ephemeral_public: [u8; 32],
    relay_static_secret: StaticSecret,
    relay_static_public: [u8; 32],
    relay_kem_secret: Zeroizing<Vec<u8>>,
    relay_kem_public: Vec<u8>,
    /// Shared secret derived when encapsulating to the client.
    relay_kem_shared: Zeroizing<Vec<u8>>,
    forward_kem_secret: Option<Zeroizing<Vec<u8>>>,
    forward_kem_public: Option<Vec<u8>>,
    forward_kem_shared: Option<Zeroizing<Vec<u8>>>,
    forward_ciphertext: Option<Vec<u8>>,
    relay_capabilities: Vec<u8>,
    descriptor_commit: Vec<u8>,
    kem_ciphertext: Vec<u8>,
    transcript_confirm_primary: Option<Vec<u8>>,
    transcript_confirm_forward: Option<Vec<u8>>,
    dual_mix: Option<Vec<u8>>,
    transcript_hash: [u8; 32],
    kem_id: u8,
    sig_id: u8,
    handshake_suite: HandshakeSuite,
    warnings: Vec<CapabilityWarning>,
    relay_hello: Vec<u8>,
    pending_session: Option<SessionSecrets>,
    requires_client_finish: bool,
}

impl RelayState {
    /// Returns `true` if the relay must await a `ClientFinish` frame before the session is usable.
    pub fn requires_client_finish(&self) -> bool {
        self.requires_client_finish
    }

    /// Transcript hash derived during the handshake.
    #[must_use]
    pub const fn transcript_hash(&self) -> &[u8; 32] {
        &self.transcript_hash
    }
}

struct ClientHelloMaterials {
    nonce: [u8; 32],
    ephemeral_secret: StaticSecret,
    ephemeral_public: [u8; NOISE_SECRET_LEN],
    static_secret: StaticSecret,
    static_public: [u8; NOISE_SECRET_LEN],
    kem_secret: Zeroizing<Vec<u8>>,
    kem_public: Vec<u8>,
}

impl ClientHelloMaterials {
    fn into_state(
        self,
        params: &RuntimeParams<'_>,
        handshake_suite: HandshakeSuite,
        client_hello: Vec<u8>,
        forward_keys: Option<(Zeroizing<Vec<u8>>, Vec<u8>)>,
    ) -> ClientState {
        let (forward_kem_secret, forward_kem_public) = match forward_keys {
            Some((secret, public)) => (Some(secret), Some(public)),
            None => (None, None),
        };

        ClientState {
            client_nonce: self.nonce,
            client_ephemeral_secret: self.ephemeral_secret,
            client_ephemeral_public: self.ephemeral_public,
            client_static_secret: self.static_secret,
            client_static_public: self.static_public,
            client_kem_secret: self.kem_secret,
            client_kem_public: self.kem_public,
            forward_kem_secret,
            forward_kem_public,
            client_capabilities: params.client_capabilities.to_vec(),
            descriptor_commit: params.descriptor_commit.to_vec(),
            resume_hash: params.resume_hash.map(<[u8]>::to_vec),
            kem_id: params.kem_id,
            sig_id: params.sig_id,
            handshake_suite,
            client_hello,
        }
    }
}

/// Build the `ClientHello` payload and return the state required for completion.
///
/// # Errors
/// Returns an error when the configured KEM profile is unsupported or when
/// constructing the handshake payload fails.
pub fn build_client_hello<R: CryptoRng + RngCore>(
    params: &RuntimeParams<'_>,
    rng: &mut R,
) -> Result<(Vec<u8>, ClientState), HarnessError> {
    let (handshake_suite, _) = verify_capabilities_alignment(
        params.kem_id,
        params.sig_id,
        params.client_capabilities,
        params.relay_capabilities,
    )?;
    let kem_profile = kem_profile(params.kem_id)?;

    let mut client_nonce = [0u8; 32];
    rng.fill_bytes(&mut client_nonce);

    let mut client_static_bytes = [0u8; NOISE_SECRET_LEN];
    rng.fill_bytes(&mut client_static_bytes);
    let client_static_secret = StaticSecret::from(client_static_bytes);
    let client_static_public = X25519PublicKey::from(&client_static_secret).to_bytes();

    let mut client_ephemeral_bytes = [0u8; NOISE_SECRET_LEN];
    rng.fill_bytes(&mut client_ephemeral_bytes);
    let client_ephemeral_secret = StaticSecret::from(client_ephemeral_bytes);
    let client_ephemeral_public = X25519PublicKey::from(&client_ephemeral_secret).to_bytes();

    let kem_keys = generate_mlkem_keypair(kem_profile.suite());
    let client_kem_public = kem_keys.public_key.clone();
    let client_kem_secret = {
        let secret = kem_keys.secret_key;
        Zeroizing::new(secret.deref().clone())
    };
    let materials = ClientHelloMaterials {
        nonce: client_nonce,
        ephemeral_secret: client_ephemeral_secret,
        ephemeral_public: client_ephemeral_public,
        static_secret: client_static_secret,
        static_public: client_static_public,
        kem_secret: client_kem_secret,
        kem_public: client_kem_public,
    };
    match (handshake_suite, materials) {
        (HandshakeSuite::Nk2Hybrid, materials) => build_client_hello_nk2(params, materials),
        (HandshakeSuite::Nk3PqForwardSecure, materials) => {
            build_client_hello_nk3(params, materials, kem_profile.suite())
        }
    }
}

fn build_client_hello_nk2(
    params: &RuntimeParams<'_>,
    materials: ClientHelloMaterials,
) -> Result<(Vec<u8>, ClientState), HarnessError> {
    let mut client_init = Vec::new();
    client_init.push(HYBRID_CLIENT_HELLO_TYPE);
    append_len_prefixed(&mut client_init, materials.nonce.as_ref());
    client_init.push(u8::from(HandshakeSuite::Nk2Hybrid));
    client_init.push(params.kem_id);
    client_init.push(params.sig_id);
    client_init.extend_from_slice(materials.ephemeral_public.as_ref());
    append_len_prefixed(&mut client_init, materials.static_public.as_ref());
    append_len_prefixed(&mut client_init, materials.kem_public.as_slice());
    append_len_prefixed(&mut client_init, params.client_capabilities);
    if let Some(resume) = params.resume_hash {
        client_init.push(1);
        append_len_prefixed(&mut client_init, resume);
    } else {
        client_init.push(0);
    }
    let client_body = client_init.clone();
    let placeholder_hash = [0u8; 32];
    let static_secret_bytes = materials.static_secret.to_bytes();
    let client_dilithium = expand_material(
        b"soranet.sig.dilithium.hybrid.client",
        &[
            static_secret_bytes.as_ref(),
            client_body.as_slice(),
            placeholder_hash.as_ref(),
        ],
        DILITHIUM3_SIGNATURE_LEN,
    );
    append_len_prefixed(&mut client_init, client_dilithium.as_slice());
    let client_ed = derive_ed25519_signature(
        b"soranet.sig.ed25519.hybrid.client",
        static_secret_bytes.as_ref(),
        &[client_body.as_slice(), placeholder_hash.as_ref()],
    )?;
    append_len_prefixed(&mut client_init, client_ed.as_ref());
    pad_to_noise_block(&mut client_init);

    let state = materials.into_state(params, HandshakeSuite::Nk2Hybrid, client_init.clone(), None);

    Ok((client_init, state))
}

fn build_client_hello_nk3(
    params: &RuntimeParams<'_>,
    materials: ClientHelloMaterials,
    kem_suite: MlKemSuite,
) -> Result<(Vec<u8>, ClientState), HarnessError> {
    let forward_keys = generate_mlkem_keypair(kem_suite);
    let forward_public = forward_keys.public_key.clone();
    let forward_secret = {
        let secret = forward_keys.secret_key;
        Zeroizing::new(secret.deref().clone())
    };
    let forward_commitment = Sha3_256::digest(forward_public.as_slice()).to_vec();

    let mut client_commit = Vec::new();
    client_commit.push(PQFS_CLIENT_COMMIT_TYPE);
    append_len_prefixed(&mut client_commit, materials.nonce.as_ref());
    client_commit.push(u8::from(HandshakeSuite::Nk3PqForwardSecure));
    client_commit.push(params.kem_id);
    client_commit.push(params.sig_id);
    client_commit.extend_from_slice(materials.ephemeral_public.as_ref());
    append_len_prefixed(&mut client_commit, materials.static_public.as_ref());
    append_len_prefixed(&mut client_commit, materials.kem_public.as_slice());
    append_len_prefixed(&mut client_commit, forward_public.as_slice());
    append_len_prefixed(&mut client_commit, params.client_capabilities);
    if let Some(resume) = params.resume_hash {
        client_commit.push(1);
        append_len_prefixed(&mut client_commit, resume);
    } else {
        client_commit.push(0);
    }
    append_len_prefixed(&mut client_commit, forward_commitment.as_slice());
    let client_body = client_commit.clone();
    let placeholder_hash = [0u8; 32];
    let static_secret_bytes = materials.static_secret.to_bytes();
    let client_dilithium = expand_material(
        b"soranet.sig.dilithium.pqfs.client",
        &[
            static_secret_bytes.as_ref(),
            client_body.as_slice(),
            placeholder_hash.as_ref(),
        ],
        DILITHIUM3_SIGNATURE_LEN,
    );
    append_len_prefixed(&mut client_commit, client_dilithium.as_slice());
    let client_ed = derive_ed25519_signature(
        b"soranet.sig.ed25519.pqfs.client",
        static_secret_bytes.as_ref(),
        &[client_body.as_slice(), placeholder_hash.as_ref()],
    )?;
    append_len_prefixed(&mut client_commit, client_ed.as_ref());
    pad_to_noise_block(&mut client_commit);

    let forward_bundle = Some((forward_secret, forward_public.clone()));
    let state = materials.into_state(
        params,
        HandshakeSuite::Nk3PqForwardSecure,
        client_commit.clone(),
        forward_bundle,
    );

    Ok((client_commit, state))
}

#[cfg(test)]
fn fixture_noise_state() -> RelayNoiseState {
    let static_secret = StaticSecret::from([0x11; NOISE_SECRET_LEN]);
    let ephemeral_secret = StaticSecret::from([0x22; NOISE_SECRET_LEN]);
    let static_public = X25519PublicKey::from(&static_secret).to_bytes();
    let ephemeral_public = X25519PublicKey::from(&ephemeral_secret).to_bytes();
    RelayNoiseState {
        nonce: [0xAA; 32],
        ephemeral_secret,
        ephemeral_public,
        static_secret,
        static_public,
    }
}

#[cfg(test)]
fn fixture_kem_artifacts(
    public: &[u8],
    secret: &[u8],
    shared: &[u8],
    ciphertext: &[u8],
) -> RuntimeKemArtifacts {
    RuntimeKemArtifacts {
        relay_public: public.to_vec(),
        relay_secret: Zeroizing::new(secret.to_vec()),
        shared_secret: Zeroizing::new(shared.to_vec()),
        ciphertext: ciphertext.to_vec(),
    }
}

#[allow(dead_code)]
struct HybridRelayParsed {
    relay_nonce: [u8; 32],
    relay_ephemeral_pub: [u8; NOISE_SECRET_LEN],
    relay_static_pub: [u8; NOISE_SECRET_LEN],
    relay_capabilities: Vec<u8>,
    descriptor_commit: Vec<u8>,
    relay_kem_public: Vec<u8>,
    kem_ciphertext: Vec<u8>,
    confirmation: Vec<u8>,
    transcript_hash: [u8; 32],
}

#[allow(dead_code)]
struct PqfsRelayParsed {
    relay_nonce: [u8; 32],
    relay_ephemeral_pub: [u8; NOISE_SECRET_LEN],
    relay_static_pub: [u8; NOISE_SECRET_LEN],
    relay_capabilities: Vec<u8>,
    descriptor_commit: Vec<u8>,
    primary_relay_public: Vec<u8>,
    primary_ciphertext: Vec<u8>,
    forward_relay_public: Vec<u8>,
    forward_ciphertext: Vec<u8>,
    primary_confirmation: Vec<u8>,
    forward_confirmation: Vec<u8>,
    transcript_hash: [u8; 32],
    forward_commitment: Vec<u8>,
    dual_mix: Vec<u8>,
}

#[allow(dead_code)]
fn parse_hybrid_relay_response(
    relay_message: &[u8],
    expected_descriptor: &[u8],
    kem_suite: MlKemSuite,
) -> Result<HybridRelayParsed, HarnessError> {
    let mut cursor = MessageCursor::new(relay_message);
    let msg_type = cursor.read_u8()?;
    if msg_type != HYBRID_RELAY_RESPONSE_TYPE {
        return Err(HarnessError::Validation(format!(
            "expected hybrid relay response type ({HYBRID_RELAY_RESPONSE_TYPE:#04x}), got {msg_type:#04x}"
        )));
    }

    let relay_nonce_bytes = cursor.read_len_prefixed()?;
    if relay_nonce_bytes.len() != 32 {
        return Err(HarnessError::Validation(
            "relay nonce must be 32 bytes".to_string(),
        ));
    }
    let mut relay_nonce = [0u8; 32];
    relay_nonce.copy_from_slice(relay_nonce_bytes);

    let relay_ephemeral_bytes = cursor.read_len_prefixed()?;
    if relay_ephemeral_bytes.len() != NOISE_SECRET_LEN {
        return Err(HarnessError::Validation(
            "relay ephemeral key must be 32 bytes".to_string(),
        ));
    }
    let mut relay_ephemeral_pub = [0u8; NOISE_SECRET_LEN];
    relay_ephemeral_pub.copy_from_slice(relay_ephemeral_bytes);

    let relay_static_bytes = cursor.read_len_prefixed()?.to_vec();
    if relay_static_bytes.len() != NOISE_SECRET_LEN {
        return Err(HarnessError::Validation(
            "relay static key must be 32 bytes".to_string(),
        ));
    }
    let mut relay_static_pub = [0u8; NOISE_SECRET_LEN];
    relay_static_pub.copy_from_slice(&relay_static_bytes);

    let relay_capabilities = cursor.read_len_prefixed()?.to_vec();
    let descriptor_commit = cursor.read_len_prefixed()?.to_vec();
    if descriptor_commit.as_slice() != expected_descriptor {
        return Err(HarnessError::Validation(format!(
            "relay descriptor commitment mismatch (expected {}, got {})",
            hex::encode(expected_descriptor),
            hex::encode(&descriptor_commit)
        )));
    }

    let relay_kem_public = cursor.read_len_prefixed()?.to_vec();
    validate_mlkem_public_key(kem_suite, &relay_kem_public)
        .map_err(|err| HarnessError::Kem(err.to_string()))?;

    let kem_ciphertext = cursor.read_len_prefixed()?.to_vec();
    validate_mlkem_ciphertext(kem_suite, &kem_ciphertext)
        .map_err(|err| HarnessError::Kem(err.to_string()))?;

    let confirmation = cursor.read_len_prefixed()?.to_vec();

    let transcript_bytes = cursor.read_len_prefixed()?.to_vec();
    if transcript_bytes.len() != 32 {
        return Err(HarnessError::Validation(
            "transcript hash must be 32 bytes".to_string(),
        ));
    }
    let mut transcript_hash = [0u8; 32];
    transcript_hash.copy_from_slice(&transcript_bytes);

    let _relay_dilithium_sig = cursor.read_len_prefixed()?;
    let _relay_ed_sig = cursor.read_len_prefixed()?;
    let _padding = cursor.remaining_slice();

    Ok(HybridRelayParsed {
        relay_nonce,
        relay_ephemeral_pub,
        relay_static_pub,
        relay_capabilities,
        descriptor_commit,
        relay_kem_public,
        kem_ciphertext,
        confirmation,
        transcript_hash,
    })
}

#[allow(dead_code)]
fn parse_pqfs_relay_response(
    relay_message: &[u8],
    expected_descriptor: &[u8],
    kem_suite: MlKemSuite,
) -> Result<PqfsRelayParsed, HarnessError> {
    let mut cursor = MessageCursor::new(relay_message);
    let msg_type = cursor.read_u8()?;
    if msg_type != PQFS_RELAY_RESPONSE_TYPE {
        return Err(HarnessError::Validation(format!(
            "expected pqfs relay response type ({PQFS_RELAY_RESPONSE_TYPE:#04x}), got {msg_type:#04x}"
        )));
    }

    let relay_nonce_bytes = cursor.read_len_prefixed()?;
    if relay_nonce_bytes.len() != 32 {
        return Err(HarnessError::Validation(
            "relay nonce must be 32 bytes".to_string(),
        ));
    }
    let mut relay_nonce = [0u8; 32];
    relay_nonce.copy_from_slice(relay_nonce_bytes);

    let relay_ephemeral_bytes = cursor.read_len_prefixed()?;
    if relay_ephemeral_bytes.len() != NOISE_SECRET_LEN {
        return Err(HarnessError::Validation(
            "relay ephemeral key must be 32 bytes".to_string(),
        ));
    }
    let mut relay_ephemeral_pub = [0u8; NOISE_SECRET_LEN];
    relay_ephemeral_pub.copy_from_slice(relay_ephemeral_bytes);

    let relay_static_bytes = cursor.read_len_prefixed()?.to_vec();
    if relay_static_bytes.len() != NOISE_SECRET_LEN {
        return Err(HarnessError::Validation(
            "relay static key must be 32 bytes".to_string(),
        ));
    }
    let mut relay_static_pub = [0u8; NOISE_SECRET_LEN];
    relay_static_pub.copy_from_slice(&relay_static_bytes);

    let relay_capabilities = cursor.read_len_prefixed()?.to_vec();
    let descriptor_commit = cursor.read_len_prefixed()?.to_vec();
    if descriptor_commit.as_slice() != expected_descriptor {
        return Err(HarnessError::Validation(format!(
            "relay descriptor commitment mismatch (expected {}, got {})",
            hex::encode(expected_descriptor),
            hex::encode(&descriptor_commit)
        )));
    }

    let primary_relay_public = cursor.read_len_prefixed()?.to_vec();
    validate_mlkem_public_key(kem_suite, &primary_relay_public)
        .map_err(|err| HarnessError::Kem(err.to_string()))?;
    let primary_ciphertext = cursor.read_len_prefixed()?.to_vec();
    validate_mlkem_ciphertext(kem_suite, &primary_ciphertext)
        .map_err(|err| HarnessError::Kem(err.to_string()))?;

    let forward_relay_public = cursor.read_len_prefixed()?.to_vec();
    validate_mlkem_public_key(kem_suite, &forward_relay_public)
        .map_err(|err| HarnessError::Kem(err.to_string()))?;
    let forward_ciphertext = cursor.read_len_prefixed()?.to_vec();
    validate_mlkem_ciphertext(kem_suite, &forward_ciphertext)
        .map_err(|err| HarnessError::Kem(err.to_string()))?;

    let primary_confirmation = cursor.read_len_prefixed()?.to_vec();
    let forward_confirmation = cursor.read_len_prefixed()?.to_vec();

    let transcript_bytes = cursor.read_len_prefixed()?.to_vec();
    if transcript_bytes.len() != 32 {
        return Err(HarnessError::Validation(
            "transcript hash must be 32 bytes".to_string(),
        ));
    }
    let mut transcript_hash = [0u8; 32];
    transcript_hash.copy_from_slice(&transcript_bytes);

    let forward_commitment = cursor.read_len_prefixed()?.to_vec();
    let dual_mix = cursor.read_len_prefixed()?.to_vec();

    let _relay_dilithium_sig = cursor.read_len_prefixed()?;
    let _relay_ed_sig = cursor.read_len_prefixed()?;
    let _padding = cursor.remaining_slice();

    Ok(PqfsRelayParsed {
        relay_nonce,
        relay_ephemeral_pub,
        relay_static_pub,
        relay_capabilities,
        descriptor_commit,
        primary_relay_public,
        primary_ciphertext,
        forward_relay_public,
        forward_ciphertext,
        primary_confirmation,
        forward_confirmation,
        transcript_hash,
        forward_commitment,
        dual_mix,
    })
}

/// Consume the relay response and derive the session key for the negotiated suite.
///
/// # Errors
/// Returns an error when the relay message is malformed, negotiates unsupported
/// capabilities, or cryptographic verification fails during the handshake.
pub fn client_handle_relay_hello<R: CryptoRng + RngCore>(
    state: ClientState,
    relay_hello: &[u8],
    _key_pair: &KeyPair,
    _params: &RuntimeParams<'_>,
    _rng: &mut R,
) -> Result<(Option<Vec<u8>>, SessionSecrets), HarnessError> {
    match state.handshake_suite {
        HandshakeSuite::Nk2Hybrid => handle_nk2_client_finish(state, relay_hello),
        HandshakeSuite::Nk3PqForwardSecure => handle_nk3_client_finish(state, relay_hello),
    }
}

fn handle_nk2_client_finish(
    state: ClientState,
    relay_message: &[u8],
) -> Result<(Option<Vec<u8>>, SessionSecrets), HarnessError> {
    let ClientState {
        client_nonce,
        client_kem_secret,
        client_capabilities,
        descriptor_commit,
        resume_hash,
        kem_id,
        sig_id,
        ..
    } = state;

    let profile = kem_profile(kem_id)?;
    let parsed = parse_hybrid_relay_response(relay_message, &descriptor_commit, profile.suite())?;

    let client_caps = parse_capabilities(&client_capabilities)?;
    let relay_caps = parse_capabilities(&parsed.relay_capabilities)?;
    let HandshakeSuiteNegotiation {
        selected,
        mut warnings,
    } = negotiate_handshake_suite(&client_caps, &relay_caps)?;
    warnings.extend(diff_capabilities(&client_caps, &relay_caps));
    if selected != HandshakeSuite::Nk2Hybrid {
        warnings.push(suite_warning(format!(
            "relay negotiated {} but NK2 Hybrid response was received",
            selected.label()
        )));
    }
    if !warnings.is_empty() {
        let telemetry = build_telemetry_payload(kem_id, sig_id, &warnings).ok();
        return Err(HarnessError::Downgrade {
            warnings,
            telemetry,
        });
    }

    let transcript_inputs = TranscriptInputs {
        descriptor_commit: parsed.descriptor_commit.as_slice(),
        client_nonce: client_nonce.as_slice(),
        relay_nonce: parsed.relay_nonce.as_slice(),
        capability_bytes: &client_capabilities,
        kem_id,
        sig_id,
        handshake_suite: HandshakeSuite::Nk2Hybrid,
        resume_hash: resume_hash.as_deref(),
    };
    let transcript = transcript_inputs.compute_hash();
    if transcript != parsed.transcript_hash {
        return Err(HarnessError::Validation(
            "relay transcript hash mismatch in NK2 handshake".to_string(),
        ));
    }

    let shared_secret = decapsulate_mlkem(
        profile.suite(),
        client_kem_secret.as_ref(),
        &parsed.kem_ciphertext,
    )
    .map_err(|err| HarnessError::Kem(err.to_string()))?;

    let (session_key, confirmation) = derive_session_key_and_confirmation(SessionKeyInputs {
        suite: HandshakeSuite::Nk2Hybrid,
        transcript_hash: &transcript,
        primary_shared: shared_secret.as_bytes(),
        forward_shared: None,
    })?;

    if confirmation != parsed.confirmation {
        return Err(HarnessError::Validation(format!(
            "relay confirmation mismatch in NK2 handshake (expected {}, got {})",
            hex::encode(&parsed.confirmation),
            hex::encode(&confirmation)
        )));
    }

    Ok((
        None,
        SessionSecrets {
            session_key,
            transcript_hash: transcript,
            handshake_suite: HandshakeSuite::Nk2Hybrid,
            warnings: Vec::new(),
            telemetry_payload: None,
        },
    ))
}

fn ensure_nk3_negotiation(
    client_capabilities: &[u8],
    relay_capabilities: &[u8],
    kem_id: u8,
    sig_id: u8,
) -> Result<(), HarnessError> {
    let client_caps = parse_capabilities(client_capabilities)?;
    let relay_caps = parse_capabilities(relay_capabilities)?;
    let HandshakeSuiteNegotiation {
        selected,
        mut warnings,
    } = negotiate_handshake_suite(&client_caps, &relay_caps)?;
    warnings.extend(diff_capabilities(&client_caps, &relay_caps));
    if selected != HandshakeSuite::Nk3PqForwardSecure {
        warnings.push(suite_warning(format!(
            "relay negotiated {} but NK3 PQ-FS response was received",
            selected.label()
        )));
    }
    if warnings.is_empty() {
        return Ok(());
    }
    let telemetry = build_telemetry_payload(kem_id, sig_id, &warnings).ok();
    Err(HarnessError::Downgrade {
        warnings,
        telemetry,
    })
}

fn compute_nk3_transcript(
    parsed: &PqfsRelayParsed,
    client_nonce: &[u8; 32],
    client_capabilities: &[u8],
    kem_id: u8,
    sig_id: u8,
    resume_hash: Option<&[u8]>,
) -> Result<[u8; 32], HarnessError> {
    let transcript_inputs = TranscriptInputs {
        descriptor_commit: parsed.descriptor_commit.as_slice(),
        client_nonce: client_nonce.as_slice(),
        relay_nonce: parsed.relay_nonce.as_slice(),
        capability_bytes: client_capabilities,
        kem_id,
        sig_id,
        handshake_suite: HandshakeSuite::Nk3PqForwardSecure,
        resume_hash,
    };
    let transcript = transcript_inputs.compute_hash();
    if transcript != parsed.transcript_hash {
        return Err(HarnessError::Validation(
            "relay transcript hash mismatch in NK3 handshake".to_string(),
        ));
    }
    Ok(transcript)
}

fn decapsulate_nk3_secrets(
    suite: MlKemSuite,
    client_secret: &Zeroizing<Vec<u8>>,
    forward_secret: &Zeroizing<Vec<u8>>,
    forward_public: &[u8],
    parsed: &PqfsRelayParsed,
    transcript: &[u8; 32],
) -> Result<(MlKemSharedSecret, MlKemSharedSecret), HarnessError> {
    let computed_commitment = Sha3_256::digest(forward_public);
    if &computed_commitment[..] != parsed.forward_commitment.as_slice() {
        return Err(HarnessError::Validation(
            "forward commitment mismatch in NK3 handshake".to_string(),
        ));
    }

    let primary_shared =
        decapsulate_mlkem(suite, client_secret.as_ref(), &parsed.primary_ciphertext)
            .map_err(|err| HarnessError::Kem(err.to_string()))?;
    let forward_shared =
        decapsulate_mlkem(suite, forward_secret.as_ref(), &parsed.forward_ciphertext)
            .map_err(|err| HarnessError::Kem(err.to_string()))?;

    let expected_primary_confirmation = compute_kem_confirmation(
        NK3_PRIMARY_CONFIRM_LABEL,
        primary_shared.as_bytes(),
        transcript,
    )?;
    if expected_primary_confirmation != parsed.primary_confirmation {
        return Err(HarnessError::Validation(format!(
            "primary confirmation mismatch in NK3 handshake (expected {}, got {})",
            hex::encode(&parsed.primary_confirmation),
            hex::encode(&expected_primary_confirmation)
        )));
    }

    let expected_forward_confirmation = compute_kem_confirmation(
        NK3_FORWARD_CONFIRM_LABEL,
        forward_shared.as_bytes(),
        transcript,
    )?;
    if expected_forward_confirmation != parsed.forward_confirmation {
        return Err(HarnessError::Validation(format!(
            "forward confirmation mismatch in NK3 handshake (expected {}, got {})",
            hex::encode(&parsed.forward_confirmation),
            hex::encode(&expected_forward_confirmation)
        )));
    }

    let expected_dual_mix = compute_dual_mix(
        primary_shared.as_bytes(),
        forward_shared.as_bytes(),
        transcript,
    );
    if expected_dual_mix != parsed.dual_mix {
        return Err(HarnessError::Validation(format!(
            "dual-mix mismatch in NK3 handshake (expected {}, got {})",
            hex::encode(&parsed.dual_mix),
            hex::encode(&expected_dual_mix)
        )));
    }

    Ok((primary_shared, forward_shared))
}

fn handle_nk3_client_finish(
    state: ClientState,
    relay_message: &[u8],
) -> Result<(Option<Vec<u8>>, SessionSecrets), HarnessError> {
    let ClientState {
        client_nonce,
        client_kem_secret,
        forward_kem_secret,
        forward_kem_public,
        client_capabilities,
        descriptor_commit,
        resume_hash,
        kem_id,
        sig_id,
        ..
    } = state;

    let forward_kem_secret = forward_kem_secret.ok_or_else(|| {
        HarnessError::Validation("forward ML-KEM secret missing in NK3 handshake".to_string())
    })?;
    let forward_public = forward_kem_public.ok_or_else(|| {
        HarnessError::Validation("forward ML-KEM public key missing in NK3 handshake".to_string())
    })?;

    let profile = kem_profile(kem_id)?;
    let parsed = parse_pqfs_relay_response(relay_message, &descriptor_commit, profile.suite())?;

    ensure_nk3_negotiation(
        &client_capabilities,
        &parsed.relay_capabilities,
        kem_id,
        sig_id,
    )?;

    let transcript = compute_nk3_transcript(
        &parsed,
        &client_nonce,
        &client_capabilities,
        kem_id,
        sig_id,
        resume_hash.as_deref(),
    )?;

    let (primary_shared, forward_shared) = decapsulate_nk3_secrets(
        profile.suite(),
        &client_kem_secret,
        &forward_kem_secret,
        &forward_public,
        &parsed,
        &transcript,
    )?;

    let (session_key, _final_confirmation) =
        derive_session_key_and_confirmation(SessionKeyInputs {
            suite: HandshakeSuite::Nk3PqForwardSecure,
            transcript_hash: &transcript,
            primary_shared: primary_shared.as_bytes(),
            forward_shared: Some(forward_shared.as_bytes()),
        })?;

    Ok((
        None,
        SessionSecrets {
            session_key,
            transcript_hash: transcript,
            handshake_suite: HandshakeSuite::Nk3PqForwardSecure,
            warnings: Vec::new(),
            telemetry_payload: None,
        },
    ))
}

struct ClientHelloParsed {
    client_nonce: [u8; 32],
    handshake_suite: HandshakeSuite,
    kem_id: u8,
    sig_id: u8,
    client_ephemeral_public: [u8; NOISE_SECRET_LEN],
    client_static_public: Option<[u8; NOISE_SECRET_LEN]>,
    client_kem_public: Vec<u8>,
    forward_kem_public: Option<Vec<u8>>,
    forward_commitment: Option<Vec<u8>>,
    client_capabilities: Vec<u8>,
    resume_hash: Option<Vec<u8>>,
}

fn parse_client_hello(
    client_hello: &[u8],
    expected_resume: Option<&[u8]>,
) -> Result<ClientHelloParsed, HarnessError> {
    let mut cursor = MessageCursor::new(client_hello);
    let msg_type = cursor.read_u8()?;
    match msg_type {
        HYBRID_CLIENT_HELLO_TYPE => parse_client_hello_nk2(cursor, expected_resume),
        PQFS_CLIENT_COMMIT_TYPE => parse_client_hello_nk3(cursor, expected_resume),
        other => Err(HarnessError::Validation(format!(
            "unexpected client hello message type {other:#04x}"
        ))),
    }
}

fn parse_client_hello_nk2(
    mut cursor: MessageCursor<'_>,
    expected_resume: Option<&[u8]>,
) -> Result<ClientHelloParsed, HarnessError> {
    let client_nonce_bytes = cursor.read_len_prefixed()?;
    if client_nonce_bytes.len() != 32 {
        return Err(HarnessError::Validation(
            "client nonce must be 32 bytes".to_string(),
        ));
    }
    let mut client_nonce = [0u8; 32];
    client_nonce.copy_from_slice(client_nonce_bytes);

    let suite_byte = cursor.read_u8()?;
    let handshake_suite = HandshakeSuite::try_from(suite_byte)?;
    if handshake_suite != HandshakeSuite::Nk2Hybrid {
        return Err(HarnessError::Validation(format!(
            "client advertised unsupported NK2 suite id {suite_byte:#04x}"
        )));
    }

    let kem_id = cursor.read_u8()?;
    let sig_id = cursor.read_u8()?;

    let mut client_ephemeral_public = [0u8; NOISE_SECRET_LEN];
    client_ephemeral_public.copy_from_slice(cursor.read_exact(NOISE_SECRET_LEN)?);

    let client_static_bytes = cursor.read_len_prefixed()?.to_vec();
    if client_static_bytes.len() != NOISE_SECRET_LEN {
        return Err(HarnessError::Validation(
            "client static key must be 32 bytes".to_string(),
        ));
    }
    let mut client_static_public = [0u8; NOISE_SECRET_LEN];
    client_static_public.copy_from_slice(&client_static_bytes);

    let client_kem_public = cursor.read_len_prefixed()?.to_vec();
    let client_capabilities = cursor.read_len_prefixed()?.to_vec();
    let resume_flag = cursor.read_u8()?;
    let resume_hash = if resume_flag == 1 {
        Some(cursor.read_len_prefixed()?.to_vec())
    } else {
        None
    };

    match (expected_resume, resume_hash.as_deref()) {
        (Some(expected), Some(actual)) if actual != expected => {
            return Err(HarnessError::Validation(format!(
                "client resume hash mismatch (expected {}, got {})",
                hex::encode(expected),
                hex::encode(actual)
            )));
        }
        (Some(_), None) => {
            return Err(HarnessError::Validation(
                "client omitted required resume hash".to_string(),
            ));
        }
        (None, Some(_)) => {
            return Err(HarnessError::Validation(
                "client supplied unexpected resume hash".to_string(),
            ));
        }
        _ => {}
    }

    let _dilithium = cursor.read_len_prefixed()?;
    let _ed = cursor.read_len_prefixed()?;
    let _padding = cursor.remaining_slice();

    Ok(ClientHelloParsed {
        client_nonce,
        handshake_suite,
        kem_id,
        sig_id,
        client_ephemeral_public,
        client_static_public: Some(client_static_public),
        client_kem_public,
        forward_kem_public: None,
        forward_commitment: None,
        client_capabilities,
        resume_hash,
    })
}

fn parse_client_hello_nk3(
    mut cursor: MessageCursor<'_>,
    expected_resume: Option<&[u8]>,
) -> Result<ClientHelloParsed, HarnessError> {
    let client_nonce_bytes = cursor.read_len_prefixed()?;
    if client_nonce_bytes.len() != 32 {
        return Err(HarnessError::Validation(
            "client nonce must be 32 bytes".to_string(),
        ));
    }
    let mut client_nonce = [0u8; 32];
    client_nonce.copy_from_slice(client_nonce_bytes);

    let suite_byte = cursor.read_u8()?;
    let handshake_suite = HandshakeSuite::try_from(suite_byte)?;
    if handshake_suite != HandshakeSuite::Nk3PqForwardSecure {
        return Err(HarnessError::Validation(format!(
            "client advertised unsupported NK3 suite id {suite_byte:#04x}"
        )));
    }

    let kem_id = cursor.read_u8()?;
    let sig_id = cursor.read_u8()?;

    let mut client_ephemeral_public = [0u8; NOISE_SECRET_LEN];
    client_ephemeral_public.copy_from_slice(cursor.read_exact(NOISE_SECRET_LEN)?);

    let client_static_bytes = cursor.read_len_prefixed()?.to_vec();
    if client_static_bytes.len() != NOISE_SECRET_LEN {
        return Err(HarnessError::Validation(
            "client static key must be 32 bytes".to_string(),
        ));
    }
    let mut client_static_public = [0u8; NOISE_SECRET_LEN];
    client_static_public.copy_from_slice(&client_static_bytes);

    let client_kem_public = cursor.read_len_prefixed()?.to_vec();
    let forward_kem_public = cursor.read_len_prefixed()?.to_vec();
    let client_capabilities = cursor.read_len_prefixed()?.to_vec();
    let resume_flag = cursor.read_u8()?;
    let resume_hash = if resume_flag == 1 {
        Some(cursor.read_len_prefixed()?.to_vec())
    } else {
        None
    };

    match (expected_resume, resume_hash.as_deref()) {
        (Some(expected), Some(actual)) if actual != expected => {
            return Err(HarnessError::Validation(format!(
                "client resume hash mismatch (expected {}, got {})",
                hex::encode(expected),
                hex::encode(actual)
            )));
        }
        (Some(_), None) => {
            return Err(HarnessError::Validation(
                "client omitted required resume hash".to_string(),
            ));
        }
        (None, Some(_)) => {
            return Err(HarnessError::Validation(
                "client supplied unexpected resume hash".to_string(),
            ));
        }
        _ => {}
    }

    let forward_commitment = cursor.read_len_prefixed()?.to_vec();
    let _dilithium = cursor.read_len_prefixed()?;
    let _ed = cursor.read_len_prefixed()?;
    let _padding = cursor.remaining_slice();

    Ok(ClientHelloParsed {
        client_nonce,
        handshake_suite,
        kem_id,
        sig_id,
        client_ephemeral_public,
        client_static_public: Some(client_static_public),
        client_kem_public,
        forward_kem_public: Some(forward_kem_public),
        forward_commitment: Some(forward_commitment),
        client_capabilities,
        resume_hash,
    })
}

struct RelayNoiseState {
    nonce: [u8; 32],
    ephemeral_secret: StaticSecret,
    ephemeral_public: [u8; NOISE_SECRET_LEN],
    static_secret: StaticSecret,
    static_public: [u8; NOISE_SECRET_LEN],
}

impl RelayNoiseState {
    fn generate<R: CryptoRng + RngCore>(rng: &mut R) -> Self {
        let mut nonce = [0u8; 32];
        rng.fill_bytes(&mut nonce);

        let mut ephemeral_bytes = [0u8; NOISE_SECRET_LEN];
        rng.fill_bytes(&mut ephemeral_bytes);
        let ephemeral_secret = StaticSecret::from(ephemeral_bytes);
        let ephemeral_public = X25519PublicKey::from(&ephemeral_secret).to_bytes();

        let mut static_bytes = [0u8; NOISE_SECRET_LEN];
        rng.fill_bytes(&mut static_bytes);
        let static_secret = StaticSecret::from(static_bytes);
        let static_public = X25519PublicKey::from(&static_secret).to_bytes();

        Self {
            nonce,
            ephemeral_secret,
            ephemeral_public,
            static_secret,
            static_public,
        }
    }
}

struct RuntimeKemArtifacts {
    relay_public: Vec<u8>,
    relay_secret: Zeroizing<Vec<u8>>,
    shared_secret: Zeroizing<Vec<u8>>,
    ciphertext: Vec<u8>,
}

impl RuntimeKemArtifacts {
    fn encapsulate(suite: MlKemSuite, client_public: &[u8]) -> Result<Self, HarnessError> {
        validate_mlkem_public_key(suite, client_public)
            .map_err(|err| HarnessError::Kem(err.to_string()))?;
        let soranet_pq::MlKemKeyPair {
            public_key: relay_public,
            secret_key: relay_secret_raw,
        } = generate_mlkem_keypair(suite);
        let relay_secret = Zeroizing::new(relay_secret_raw.deref().clone());
        let (shared_secret, ciphertext) = encapsulate_mlkem(suite, client_public)
            .map_err(|err| HarnessError::Kem(err.to_string()))?;
        Ok(Self {
            relay_public,
            relay_secret,
            shared_secret: Zeroizing::new(shared_secret.as_bytes().to_vec()),
            ciphertext: ciphertext.as_bytes().to_vec(),
        })
    }
}

fn ensure_kem_profile(
    params: &RuntimeParams<'_>,
    requested: u8,
) -> Result<KemProfile, HarnessError> {
    if params.kem_id != requested {
        return Err(HarnessError::Validation(format!(
            "client requested ML-KEM id {requested}, but relay is configured for {}",
            params.kem_id
        )));
    }
    kem_profile(requested)
}

fn verify_capabilities_alignment(
    kem_id: u8,
    sig_id: u8,
    client_capabilities: &[u8],
    relay_capabilities: &[u8],
) -> Result<(HandshakeSuite, Vec<CapabilityWarning>), HarnessError> {
    let client_caps = parse_capabilities(client_capabilities)?;
    let relay_caps = parse_capabilities(relay_capabilities)?;
    let HandshakeSuiteNegotiation {
        selected,
        mut warnings,
    } = negotiate_handshake_suite(&client_caps, &relay_caps)?;
    warnings.extend(diff_capabilities(&client_caps, &relay_caps));
    if warnings.is_empty() {
        Ok((selected, warnings))
    } else {
        let telemetry = build_telemetry_payload(kem_id, sig_id, &warnings).ok();
        Err(HarnessError::Downgrade {
            warnings,
            telemetry,
        })
    }
}

fn compute_relay_transcript(
    parsed: &ClientHelloParsed,
    relay_nonce: &[u8; 32],
    params: &RuntimeParams<'_>,
    handshake_suite: HandshakeSuite,
) -> [u8; 32] {
    TranscriptInputs {
        descriptor_commit: params.descriptor_commit,
        client_nonce: parsed.client_nonce.as_slice(),
        relay_nonce: relay_nonce.as_slice(),
        capability_bytes: &parsed.client_capabilities,
        kem_id: parsed.kem_id,
        sig_id: parsed.sig_id,
        handshake_suite,
        resume_hash: parsed.resume_hash.as_deref(),
    }
    .compute_hash()
}

#[allow(dead_code)]
fn build_hybrid_relay_response(
    client_init: &[u8],
    params: &RuntimeParams<'_>,
    noise: &RelayNoiseState,
    primary: &RuntimeKemArtifacts,
    confirmation: &[u8],
    transcript: &[u8; 32],
) -> Result<Vec<u8>, HarnessError> {
    let mut relay_response = Vec::new();
    relay_response.push(HYBRID_RELAY_RESPONSE_TYPE);
    append_len_prefixed(&mut relay_response, &noise.nonce);
    append_len_prefixed(&mut relay_response, &noise.ephemeral_public);
    append_len_prefixed(&mut relay_response, &noise.static_public);
    append_len_prefixed(&mut relay_response, params.relay_capabilities);
    append_len_prefixed(&mut relay_response, params.descriptor_commit);
    append_len_prefixed(&mut relay_response, &primary.relay_public);
    append_len_prefixed(&mut relay_response, &primary.ciphertext);
    append_len_prefixed(&mut relay_response, confirmation);
    append_len_prefixed(&mut relay_response, transcript.as_ref());

    let relay_body = relay_response.clone();
    let relay_dilithium = expand_material(
        b"soranet.sig.dilithium.hybrid.relay",
        &[
            noise.static_secret.to_bytes().as_ref(),
            client_init,
            relay_body.as_slice(),
            transcript.as_ref(),
        ],
        DILITHIUM3_SIGNATURE_LEN,
    );
    append_len_prefixed(&mut relay_response, &relay_dilithium);
    let relay_ed = derive_ed25519_signature(
        b"soranet.sig.ed25519.hybrid.relay",
        noise.static_secret.to_bytes().as_ref(),
        &[client_init, relay_body.as_slice(), transcript.as_ref()],
    )?;
    append_len_prefixed(&mut relay_response, &relay_ed);
    pad_to_noise_block(&mut relay_response);
    Ok(relay_response)
}

/// Aggregates the inputs needed to serialize a PQFS relay response.
struct PqfsRelayResponseInputs<'ctx> {
    client_commit: &'ctx [u8],
    params: &'ctx RuntimeParams<'ctx>,
    noise: &'ctx RelayNoiseState,
    primary: &'ctx RuntimeKemArtifacts,
    forward: &'ctx RuntimeKemArtifacts,
    primary_confirmation: &'ctx [u8],
    forward_confirmation: &'ctx [u8],
    transcript: &'ctx [u8; 32],
    forward_commitment: &'ctx [u8],
    dual_mix: &'ctx [u8],
}

#[allow(dead_code)]
fn build_pqfs_relay_response(
    inputs: &PqfsRelayResponseInputs<'_>,
) -> Result<Vec<u8>, HarnessError> {
    let noise = inputs.noise;
    let params = inputs.params;
    let primary = inputs.primary;
    let forward = inputs.forward;

    let mut relay_response = Vec::new();
    relay_response.push(PQFS_RELAY_RESPONSE_TYPE);
    append_len_prefixed(&mut relay_response, &noise.nonce);
    append_len_prefixed(&mut relay_response, &noise.ephemeral_public);
    append_len_prefixed(&mut relay_response, &noise.static_public);
    append_len_prefixed(&mut relay_response, params.relay_capabilities);
    append_len_prefixed(&mut relay_response, params.descriptor_commit);
    append_len_prefixed(&mut relay_response, &primary.relay_public);
    append_len_prefixed(&mut relay_response, &primary.ciphertext);
    append_len_prefixed(&mut relay_response, &forward.relay_public);
    append_len_prefixed(&mut relay_response, &forward.ciphertext);
    append_len_prefixed(&mut relay_response, inputs.primary_confirmation);
    append_len_prefixed(&mut relay_response, inputs.forward_confirmation);
    append_len_prefixed(&mut relay_response, inputs.transcript.as_ref());
    append_len_prefixed(&mut relay_response, inputs.forward_commitment);
    append_len_prefixed(&mut relay_response, inputs.dual_mix);

    let relay_body = relay_response.clone();
    let relay_dilithium = expand_material(
        b"soranet.sig.dilithium.pqfs.relay",
        &[
            noise.static_secret.to_bytes().as_ref(),
            inputs.client_commit,
            relay_body.as_slice(),
            inputs.transcript.as_ref(),
        ],
        DILITHIUM3_SIGNATURE_LEN,
    );
    append_len_prefixed(&mut relay_response, &relay_dilithium);
    let relay_ed = derive_ed25519_signature(
        b"soranet.sig.ed25519.pqfs.relay",
        noise.static_secret.to_bytes().as_ref(),
        &[
            inputs.client_commit,
            relay_body.as_slice(),
            inputs.transcript.as_ref(),
        ],
    )?;
    append_len_prefixed(&mut relay_response, &relay_ed);
    pad_to_noise_block(&mut relay_response);
    Ok(relay_response)
}

struct RelayStateInputs<'params, 'runtime> {
    parsed: ClientHelloParsed,
    params: &'params RuntimeParams<'runtime>,
    noise: RelayNoiseState,
    kem: RuntimeKemArtifacts,
    transcript: [u8; 32],
    handshake_suite: HandshakeSuite,
    warnings: Vec<CapabilityWarning>,
    relay_hello: Vec<u8>,
}

fn assemble_relay_state(inputs: RelayStateInputs<'_, '_>) -> RelayState {
    let RelayStateInputs {
        parsed,
        params,
        noise,
        kem,
        transcript,
        handshake_suite,
        warnings,
        relay_hello,
    } = inputs;

    let ClientHelloParsed {
        client_nonce,
        client_static_public,
        kem_id,
        sig_id,
        client_ephemeral_public,
        client_kem_public,
        client_capabilities,
        resume_hash,
        ..
    } = parsed;

    RelayState {
        client_nonce,
        client_ephemeral_public,
        client_capabilities,
        client_kem_public,
        resume_hash,
        client_static_public,
        relay_nonce: noise.nonce,
        relay_ephemeral_secret: noise.ephemeral_secret,
        relay_ephemeral_public: noise.ephemeral_public,
        relay_static_secret: noise.static_secret,
        relay_static_public: noise.static_public,
        relay_kem_secret: kem.relay_secret,
        relay_kem_public: kem.relay_public,
        relay_kem_shared: kem.shared_secret,
        forward_kem_secret: None,
        forward_kem_public: None,
        forward_kem_shared: None,
        forward_ciphertext: None,
        relay_capabilities: params.relay_capabilities.to_vec(),
        descriptor_commit: params.descriptor_commit.to_vec(),
        kem_ciphertext: kem.ciphertext,
        transcript_confirm_primary: None,
        transcript_confirm_forward: None,
        dual_mix: None,
        transcript_hash: transcript,
        kem_id,
        sig_id,
        handshake_suite,
        warnings,
        relay_hello,
        pending_session: None,
        requires_client_finish: false,
    }
}

#[allow(dead_code)]
fn process_nk2_client_hello<R: CryptoRng + RngCore>(
    client_init: &[u8],
    parsed: ClientHelloParsed,
    params: &RuntimeParams<'_>,
    rng: &mut R,
    kem_suite: MlKemSuite,
) -> Result<(Vec<u8>, RelayState), HarnessError> {
    if parsed.client_static_public.is_none() {
        return Err(HarnessError::Validation(
            "NK2 handshake requires client static key".into(),
        ));
    }
    let client_forward = parsed.forward_kem_public.clone();
    if client_forward.is_some() {
        return Err(HarnessError::Validation(
            "NK2 handshake must not include forward KEM payload".into(),
        ));
    }

    let noise = RelayNoiseState::generate(rng);
    let primary = RuntimeKemArtifacts::encapsulate(kem_suite, &parsed.client_kem_public)?;
    let transcript =
        compute_relay_transcript(&parsed, &noise.nonce, params, HandshakeSuite::Nk2Hybrid);

    let (session_key, confirmation) = derive_session_key_and_confirmation(SessionKeyInputs {
        suite: HandshakeSuite::Nk2Hybrid,
        transcript_hash: &transcript,
        primary_shared: primary.shared_secret.as_ref(),
        forward_shared: None,
    })?;

    let relay_response = build_hybrid_relay_response(
        client_init,
        params,
        &noise,
        &primary,
        &confirmation,
        &transcript,
    )?;

    let mut state = assemble_relay_state(RelayStateInputs {
        parsed,
        params,
        noise,
        kem: primary,
        transcript,
        handshake_suite: HandshakeSuite::Nk2Hybrid,
        warnings: Vec::new(),
        relay_hello: relay_response.clone(),
    });
    state.transcript_confirm_primary = Some(confirmation.clone());
    state.pending_session = Some(SessionSecrets {
        session_key,
        transcript_hash: transcript,
        handshake_suite: HandshakeSuite::Nk2Hybrid,
        warnings: Vec::new(),
        telemetry_payload: None,
    });
    state.requires_client_finish = false;

    Ok((relay_response, state))
}

/// Required payloads carried by the NK3 client hello message.
struct Nk3HandshakeRequirements {
    forward_public: Vec<u8>,
    forward_commitment: Vec<u8>,
}

impl Nk3HandshakeRequirements {
    fn collect(parsed: &ClientHelloParsed) -> Result<Self, HarnessError> {
        let _client_static_public = parsed.client_static_public.ok_or_else(|| {
            HarnessError::Validation("NK3 handshake requires client static key".into())
        })?;
        let forward_public = parsed.forward_kem_public.clone().ok_or_else(|| {
            HarnessError::Validation("NK3 handshake requires forward ML-KEM public key".into())
        })?;
        let forward_commitment = parsed.forward_commitment.clone().ok_or_else(|| {
            HarnessError::Validation("NK3 handshake requires forward commitment".into())
        })?;
        Ok(Self {
            forward_public,
            forward_commitment,
        })
    }
}

/// ML-KEM confirmation tags and dual-mix secret derived for NK3.
struct Nk3ConfirmationBundle {
    primary: Vec<u8>,
    forward: Vec<u8>,
    dual_mix: Vec<u8>,
}

impl Nk3ConfirmationBundle {
    fn derive(
        primary_shared: &[u8],
        forward_shared: &[u8],
        transcript: &[u8; 32],
    ) -> Result<Self, HarnessError> {
        let primary =
            compute_kem_confirmation(NK3_PRIMARY_CONFIRM_LABEL, primary_shared, transcript)?;
        let forward =
            compute_kem_confirmation(NK3_FORWARD_CONFIRM_LABEL, forward_shared, transcript)?;
        let dual_mix = compute_dual_mix(primary_shared, forward_shared, transcript);
        Ok(Self {
            primary,
            forward,
            dual_mix,
        })
    }
}

#[allow(dead_code)]
fn process_nk3_client_hello<R: CryptoRng + RngCore>(
    client_commit: &[u8],
    parsed: ClientHelloParsed,
    params: &RuntimeParams<'_>,
    rng: &mut R,
    kem_suite: MlKemSuite,
) -> Result<(Vec<u8>, RelayState), HarnessError> {
    let requirements = Nk3HandshakeRequirements::collect(&parsed)?;

    let noise = RelayNoiseState::generate(rng);
    let primary = RuntimeKemArtifacts::encapsulate(kem_suite, &parsed.client_kem_public)?;
    let forward = RuntimeKemArtifacts::encapsulate(kem_suite, &requirements.forward_public)?;

    let transcript = compute_relay_transcript(
        &parsed,
        &noise.nonce,
        params,
        HandshakeSuite::Nk3PqForwardSecure,
    );

    let confirmations = Nk3ConfirmationBundle::derive(
        primary.shared_secret.as_ref(),
        forward.shared_secret.as_ref(),
        &transcript,
    )?;

    let (session_key, _) = derive_session_key_and_confirmation(SessionKeyInputs {
        suite: HandshakeSuite::Nk3PqForwardSecure,
        transcript_hash: &transcript,
        primary_shared: primary.shared_secret.as_ref(),
        forward_shared: Some(forward.shared_secret.as_ref()),
    })?;

    let response_inputs = PqfsRelayResponseInputs {
        client_commit,
        params,
        noise: &noise,
        primary: &primary,
        forward: &forward,
        primary_confirmation: confirmations.primary.as_slice(),
        forward_confirmation: confirmations.forward.as_slice(),
        transcript: &transcript,
        forward_commitment: requirements.forward_commitment.as_slice(),
        dual_mix: confirmations.dual_mix.as_slice(),
    };
    let relay_response = build_pqfs_relay_response(&response_inputs)?;

    let mut state = assemble_relay_state(RelayStateInputs {
        parsed,
        params,
        noise,
        kem: primary,
        transcript,
        handshake_suite: HandshakeSuite::Nk3PqForwardSecure,
        warnings: Vec::new(),
        relay_hello: relay_response.clone(),
    });

    let RuntimeKemArtifacts {
        relay_public: forward_relay_public,
        relay_secret: forward_relay_secret,
        shared_secret: forward_shared_secret,
        ciphertext: forward_ciphertext,
    } = forward;

    state.forward_kem_secret = Some(forward_relay_secret);
    state.forward_kem_public = Some(forward_relay_public);
    state.forward_kem_shared = Some(forward_shared_secret);
    state.forward_ciphertext = Some(forward_ciphertext);
    state.transcript_confirm_primary = Some(confirmations.primary.clone());
    state.transcript_confirm_forward = Some(confirmations.forward.clone());
    state.dual_mix = Some(confirmations.dual_mix.clone());
    state.pending_session = Some(SessionSecrets {
        session_key,
        transcript_hash: transcript,
        handshake_suite: HandshakeSuite::Nk3PqForwardSecure,
        warnings: Vec::new(),
        telemetry_payload: None,
    });
    state.requires_client_finish = false;

    Ok((relay_response, state))
}

/// Parse an incoming `ClientHello` and craft the `RelayHello` response.
///
/// # Errors
/// Returns an error when the client message is malformed, capabilities are
/// incompatible, or cryptographic material fails validation.
pub fn process_client_hello<R: CryptoRng + RngCore>(
    client_hello: &[u8],
    params: &RuntimeParams<'_>,
    _key_pair: &KeyPair,
    rng: &mut R,
) -> Result<(Vec<u8>, RelayState), HarnessError> {
    let parsed = parse_client_hello(client_hello, params.resume_hash)?;
    let profile = ensure_kem_profile(params, parsed.kem_id)?;
    let (negotiated_suite, _warnings) = verify_capabilities_alignment(
        parsed.kem_id,
        parsed.sig_id,
        &parsed.client_capabilities,
        params.relay_capabilities,
    )?;
    if negotiated_suite != parsed.handshake_suite {
        let warning = suite_warning(format!(
            "client advertised {}, but relay policy selected {}",
            parsed.handshake_suite.label(),
            negotiated_suite.label()
        ));
        let telemetry =
            build_telemetry_payload(parsed.kem_id, parsed.sig_id, std::slice::from_ref(&warning))
                .ok();
        return Err(HarnessError::Downgrade {
            warnings: vec![warning],
            telemetry,
        });
    }

    match parsed.handshake_suite {
        HandshakeSuite::Nk2Hybrid => {
            process_nk2_client_hello(client_hello, parsed, params, rng, profile.suite())
        }
        HandshakeSuite::Nk3PqForwardSecure => {
            process_nk3_client_hello(client_hello, parsed, params, rng, profile.suite())
        }
    }
}

/// Finalise the relay side of the handshake after the relay response is sent.
///
/// # Errors
/// Returns an error if the handshake expects a client-finish message.
pub fn relay_finalize_handshake(
    state: RelayState,
    _client_finish: &[u8],
    _key_pair: &KeyPair,
) -> Result<SessionSecrets, HarnessError> {
    if !state.requires_client_finish {
        return state
            .pending_session
            .ok_or_else(|| HarnessError::Validation("handshake already finalised".into()));
    }
    Err(HarnessError::Validation(
        "client-finish handshakes are no longer supported".into(),
    ))
}

fn build_telemetry_payload(
    kem_id: u8,
    sig_id: u8,
    warnings: &[CapabilityWarning],
) -> Result<Vec<u8>, HarnessError> {
    let downgrade_attempts = u32::try_from(warnings.len()).map_err(|_| {
        HarnessError::Validation(format!(
            "downgrade warning count {} exceeds supported range",
            warnings.len()
        ))
    })?;
    let report = TelemetryReport {
        epoch: 0,
        downgrade_attempts,
        pq_disabled_sessions: 0,
        cover_ratio: 0.0,
        lagging_clients: 0,
        max_latency_ms: 0,
        incident_reference: None,
        signature: None,
        witness_signature: None,
    };
    let mut report_map = Map::new();
    report_map.insert("epoch".to_string(), Value::from(report.epoch));
    report_map.insert(
        "downgrade_attempts".to_string(),
        Value::from(report.downgrade_attempts),
    );
    report_map.insert(
        "pq_disabled_sessions".to_string(),
        Value::from(report.pq_disabled_sessions),
    );
    report_map.insert(
        "cover_ratio".to_string(),
        Value::from(f64::from(report.cover_ratio)),
    );
    report_map.insert(
        "lagging_clients".to_string(),
        Value::from(report.lagging_clients),
    );
    report_map.insert(
        "max_latency_ms".to_string(),
        Value::from(report.max_latency_ms),
    );
    report_map.insert(
        "incident_reference".to_string(),
        option_str_to_value(report.incident_reference),
    );
    report_map.insert(
        "signature".to_string(),
        option_str_to_value(report.signature),
    );
    report_map.insert(
        "witness_signature".to_string(),
        option_str_to_value(report.witness_signature),
    );

    let mut map = Map::new();
    map.insert("event".to_string(), Value::from("soranet_handshake"));
    map.insert("kem_id".to_string(), Value::from(kem_id));
    map.insert("sig_id".to_string(), Value::from(sig_id));
    map.insert("warning_count".to_string(), Value::from(warnings.len()));
    map.insert("report".to_string(), Value::Object(report_map));
    json::to_string_pretty(&Value::Object(map))
        .map(String::into_bytes)
        .map_err(HarnessError::from)
}

/// Inputs required to derive session keys and confirmation tags.
#[derive(Clone, Copy)]
struct SessionKeyInputs<'a> {
    suite: HandshakeSuite,
    transcript_hash: &'a [u8; 32],
    primary_shared: &'a [u8],
    forward_shared: Option<&'a [u8]>,
}

fn derive_session_key_and_confirmation(
    inputs: SessionKeyInputs<'_>,
) -> Result<(Vec<u8>, Vec<u8>), HarnessError> {
    let SessionKeyInputs {
        suite,
        transcript_hash,
        primary_shared,
        forward_shared,
    } = inputs;

    let (session_label, confirm_label) = match suite {
        HandshakeSuite::Nk2Hybrid => (
            b"soranet.handshake.nk2.session",
            b"soranet.handshake.nk2.confirm",
        ),
        HandshakeSuite::Nk3PqForwardSecure => (
            b"soranet.handshake.nk3.session",
            b"soranet.handshake.nk3.confirm",
        ),
    };

    let key_material = match suite {
        HandshakeSuite::Nk2Hybrid => {
            let mut material = Vec::with_capacity(primary_shared.len() + transcript_hash.len());
            material.extend_from_slice(primary_shared);
            material.extend_from_slice(transcript_hash);
            material
        }
        HandshakeSuite::Nk3PqForwardSecure => {
            let forward = forward_shared.ok_or(HarnessError::NotImplemented(
                "nk3 forward-secure key schedule requires dual ML-KEM secret",
            ))?;
            let dual_mix = expand_material(
                b"soranet.kem.dual.mix",
                &[primary_shared, forward, transcript_hash],
                forward.len(),
            );
            let mut material = Vec::with_capacity(
                dual_mix.len() + primary_shared.len() + forward.len() + transcript_hash.len(),
            );
            material.extend_from_slice(&dual_mix);
            material.extend_from_slice(primary_shared);
            material.extend_from_slice(forward);
            material.extend_from_slice(transcript_hash);
            material
        }
    };

    let hk = Hkdf::<Sha3_256>::new(Some(transcript_hash), &key_material);
    let mut session_key = vec![0u8; 32];
    hk.expand(session_label, &mut session_key)
        .map_err(|_| HarnessError::Kdf)?;

    let confirmation = if suite == HandshakeSuite::Nk2Hybrid {
        let mut confirm = vec![0u8; 32];
        let hk_confirm = Hkdf::<Sha3_256>::new(Some(transcript_hash), primary_shared);
        hk_confirm
            .expand(NK2_CONFIRM_LABEL, &mut confirm)
            .map_err(|_| HarnessError::Kdf)?;
        confirm
    } else {
        let mut confirm = vec![0u8; 32];
        hk.expand(confirm_label, &mut confirm)
            .map_err(|_| HarnessError::Kdf)?;
        confirm
    };

    Ok((session_key, confirmation))
}

fn compute_kem_confirmation(
    label: &[u8],
    shared_secret: &[u8],
    transcript_hash: &[u8; 32],
) -> Result<Vec<u8>, HarnessError> {
    let hk = Hkdf::<Sha3_256>::new(Some(transcript_hash), shared_secret);
    let mut confirmation = vec![0u8; shared_secret.len()];
    hk.expand(label, &mut confirmation)
        .map_err(|_| HarnessError::Kdf)?;
    Ok(confirmation)
}

fn compute_dual_mix(
    primary_shared: &[u8],
    forward_shared: &[u8],
    transcript_hash: &[u8; 32],
) -> Vec<u8> {
    expand_material(
        b"soranet.kem.dual.mix",
        &[primary_shared, forward_shared, transcript_hash],
        forward_shared.len(),
    )
}

struct MessageCursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> MessageCursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn read_exact(&mut self, len: usize) -> Result<&'a [u8], HarnessError> {
        if self.pos + len > self.buf.len() {
            let remaining = self.buf.len().saturating_sub(self.pos);
            return Err(HarnessError::Validation(format!(
                "handshake message truncated (offset={}, need={}, remaining={remaining})",
                self.pos, len
            )));
        }
        let slice = &self.buf[self.pos..self.pos + len];
        self.pos += len;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8, HarnessError> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_len_prefixed(&mut self) -> Result<&'a [u8], HarnessError> {
        let len_bytes = self.read_exact(2)?;
        let len = u16::from_be_bytes([len_bytes[0], len_bytes[1]]) as usize;
        self.read_exact(len)
    }

    fn remaining_slice(&self) -> &[u8] {
        &self.buf[self.pos..]
    }
}

#[cfg(test)]
mod tests {
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;
    #[test]
    fn update_suite_list_sets_required_flag() {
        let suites = [HandshakeSuite::Nk3PqForwardSecure];
        let vector = super::update_suite_list(&DEFAULT_CLIENT_CAPABILITIES, &suites, true).unwrap();
        let parsed = parse_capabilities(&vector).expect("parse updated capabilities");
        let suite_cap = parsed
            .into_iter()
            .find(|cap| cap.ty == CAPABILITY_SUITE_LIST)
            .expect("suite_list capability present");
        assert_eq!(
            suite_cap.value,
            vec![u8::from(HandshakeSuite::Nk3PqForwardSecure)]
        );
        assert!(suite_cap.required, "required flag propagated");
    }

    #[test]
    fn parse_suite_list_ignores_unknown_ids() {
        let cap = CapabilityTlv {
            ty: CAPABILITY_SUITE_LIST,
            value: vec![
                u8::from(HandshakeSuite::Nk2Hybrid),
                0x01,
                u8::from(HandshakeSuite::Nk3PqForwardSecure),
                0x01,
            ],
            required: false,
        };
        let list = parse_suite_list(&cap).expect("parse suite list");
        assert_eq!(
            list.suites,
            vec![
                HandshakeSuite::Nk2Hybrid,
                HandshakeSuite::Nk3PqForwardSecure
            ]
        );
        assert!(!list.required);
    }

    #[test]
    fn parse_capabilities_rejects_unknown_type() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&0x0301u16.to_be_bytes());
        buf.extend_from_slice(&1u16.to_be_bytes());
        buf.push(0);
        let err = parse_capabilities(&buf).expect_err("unknown capability should fail");
        assert!(matches!(err, HarnessError::CapabilityType(_)));
    }

    #[test]
    fn parse_capabilities_rejects_duplicate_singleton() {
        let entries = vec![
            CapabilityTlv {
                ty: CAPABILITY_ROLE,
                value: vec![0x01],
                required: false,
            },
            CapabilityTlv {
                ty: CAPABILITY_ROLE,
                value: vec![0x02],
                required: false,
            },
        ];
        let buf = encode_tlvs(&entries);
        let err = parse_capabilities(&buf).expect_err("duplicate role should fail");
        assert!(matches!(err, HarnessError::DuplicateCapability(_)));
    }

    #[test]
    fn parse_capabilities_allows_duplicate_pqsig() {
        let entries = vec![
            CapabilityTlv {
                ty: CAPABILITY_PQSIG,
                value: vec![0x01, 0x00],
                required: false,
            },
            CapabilityTlv {
                ty: CAPABILITY_PQSIG,
                value: vec![0x02, 0x00],
                required: false,
            },
        ];
        let buf = encode_tlvs(&entries);
        let parsed = parse_capabilities(&buf).expect("pqsig duplicates allowed");
        let pqsigs: Vec<_> = parsed
            .iter()
            .filter(|cap| cap.ty == CAPABILITY_PQSIG)
            .collect();
        assert_eq!(pqsigs.len(), 2);
        assert_eq!(pqsigs[0].value, vec![0x01, 0x00]);
        assert_eq!(pqsigs[1].value, vec![0x02, 0x00]);
    }

    #[test]
    fn build_interop_value_exposes_session_key() {
        for spec in super::INTEROP_SPECS {
            let value = super::build_interop_value(spec).expect("interop vector");
            let root = value
                .as_object()
                .expect("interop value should be a JSON object");
            assert_eq!(
                root.get("suite").and_then(Value::as_str),
                Some(spec.suite.label())
            );
            let outputs = root
                .get("outputs")
                .and_then(Value::as_object)
                .expect("outputs object present");
            let session_key = outputs
                .get("session_key_hex")
                .and_then(Value::as_str)
                .expect("session key present");
            assert_eq!(
                session_key.len(),
                64,
                "session key hex should be 32 bytes (64 hex chars)"
            );
        }
    }

    #[test]
    fn prepare_capability_context_matches_defaults() {
        let spec = &super::INTEROP_SPECS[0];
        let ctx = super::prepare_capability_context(spec).expect("capability context");
        assert_eq!(
            ctx.client_caps.len(),
            super::DEFAULT_CLIENT_CAPABILITIES.len()
        );
        assert_eq!(
            ctx.relay_caps.len(),
            super::DEFAULT_RELAY_CAPABILITIES.len()
        );
        assert!(ctx.warnings.is_empty(), "expected no negotiation warnings");
        assert!(
            !ctx.client_tlvs.is_empty() && !ctx.relay_tlvs.is_empty(),
            "parsed TLVs should not be empty"
        );
    }

    #[test]
    fn decode_handshake_inputs_handles_resume_hash() {
        let base = &super::INTEROP_SPECS[0];
        let resume_hex = "aabbccddeeff00112233445566778899";
        let spec = super::InteropSpec {
            id: "resume-test",
            description: "spec exercising resume hash decoding",
            suite: base.suite,
            client_suites: base.client_suites,
            client_required: base.client_required,
            relay_suites: base.relay_suites,
            relay_required: base.relay_required,
            client_static_sk_hex: base.client_static_sk_hex,
            relay_static_sk_hex: base.relay_static_sk_hex,
            client_nonce_hex: base.client_nonce_hex,
            relay_nonce_hex: base.relay_nonce_hex,
            resume_hash_hex: Some(resume_hex),
            kem_id: base.kem_id,
            sig_id: base.sig_id,
        };

        let inputs = super::decode_handshake_inputs(&spec).expect("decoded inputs");
        assert_eq!(
            inputs.client_static.to_vec(),
            super::decode_hex(base.client_static_sk_hex)
                .expect("decode baseline client static key")
        );
        let resume = inputs.resume_hash.expect("resume hash decoded");
        assert_eq!(
            resume,
            super::decode_hex(resume_hex).expect("decode resume hex")
        );
    }

    #[test]
    fn build_session_artifacts_emits_forward_shared_for_nk3() {
        let spec = &super::INTEROP_SPECS[1];
        let ctx = super::prepare_capability_context(spec).expect("capability context");
        let inputs = super::decode_handshake_inputs(spec).expect("decoded inputs");
        let params = super::build_simulation_params(spec, &ctx, &inputs);
        let transcript = super::compute_transcript_hash(&params, spec.suite);
        let material =
            super::derive_handshake_material(&params, &inputs.client_static, &inputs.relay_static)
                .expect("handshake material");

        let session =
            super::build_session_artifacts(spec, &params, &transcript, &material, &ctx.warnings)
                .expect("session artifacts");

        assert!(
            session.forward_shared.is_some(),
            "NK3 should derive forward secret"
        );
        assert!(
            session.dual_mix.is_some(),
            "NK3 should derive dual mix material"
        );
        assert_eq!(session.session_key.len(), 32, "session key length");
        assert!(
            !session.handshake.steps.is_empty(),
            "handshake steps should not be empty"
        );
    }

    struct Nk3Fixture {
        client_state: ClientState,
        relay_response: PqfsRelayParsed,
        suite: MlKemSuite,
        client_caps: Vec<u8>,
        relay_caps: Vec<u8>,
        kem_id: u8,
        sig_id: u8,
    }

    fn build_nk3_fixture() -> Nk3Fixture {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let (client_state, parsed, suite) = {
            let params = RuntimeParams {
                descriptor_commit: defaults.descriptor_commit,
                client_capabilities: client_caps.as_slice(),
                relay_capabilities: relay_caps.as_slice(),
                kem_id: defaults.kem_id,
                sig_id: defaults.sig_id,
                resume_hash: defaults.resume_hash,
            };

            let mut rng_client = StdRng::seed_from_u64(2024);
            let mut rng_relay = StdRng::seed_from_u64(4048);
            let relay_keys = KeyPair::random();

            let (client_hello, client_state) =
                build_client_hello(&params, &mut rng_client).expect("nk3 client hello");
            let (relay_hello, _relay_state) =
                process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                    .expect("nk3 relay response");

            let profile = kem_profile(params.kem_id).expect("kem profile");
            let parsed =
                parse_pqfs_relay_response(&relay_hello, params.descriptor_commit, profile.suite())
                    .expect("parse nk3 response");

            (client_state, parsed, profile.suite())
        };

        Nk3Fixture {
            client_state,
            relay_response: parsed,
            suite,
            client_caps,
            relay_caps,
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
        }
    }

    fn sample_static_keys() -> (Vec<u8>, Vec<u8>) {
        let client_static =
            decode_hex("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff")
                .expect("client static hex");
        let relay_static =
            decode_hex("ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100")
                .expect("relay static hex");
        (client_static, relay_static)
    }

    fn suite_list_tlv(required: bool, suites: &[HandshakeSuite]) -> Vec<u8> {
        assert!(
            !suites.is_empty(),
            "suite list helper expects at least one entry"
        );
        let mut values: Vec<u8> = suites.iter().copied().map(u8::from).collect();
        if required {
            values[0] |= 0x80;
        }
        let mut buf = Vec::new();
        buf.extend_from_slice(&CAPABILITY_SUITE_LIST.to_be_bytes());
        let len = u16::try_from(values.len()).expect("suite list length fits u16");
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&values);
        buf
    }

    fn encode_tlvs(entries: &[CapabilityTlv]) -> Vec<u8> {
        let mut buf = Vec::new();
        for cap in entries {
            let mut value = cap.value.clone();
            apply_required_flag(cap.ty, &mut value, cap.required);
            buf.extend_from_slice(&cap.ty.to_be_bytes());
            let len = u16::try_from(value.len()).expect("capability value fits u16");
            buf.extend_from_slice(&len.to_be_bytes());
            buf.extend_from_slice(&value);
        }
        buf
    }

    fn capabilities_with_suites(base: &[u8], suites: &[HandshakeSuite], required: bool) -> Vec<u8> {
        assert!(
            !suites.is_empty(),
            "suite list helper expects at least one entry"
        );
        let mut entries = parse_capabilities(base).expect("parse base capabilities");
        let value: Vec<u8> = suites.iter().copied().map(u8::from).collect();
        if let Some(existing) = entries
            .iter_mut()
            .find(|cap| cap.ty == CAPABILITY_SUITE_LIST)
        {
            existing.value = value;
            existing.required = required;
        } else {
            entries.push(CapabilityTlv {
                ty: CAPABILITY_SUITE_LIST,
                value,
                required,
            });
        }
        entries.sort_by_key(|cap| cap.ty);
        encode_tlvs(&entries)
    }

    #[test]
    fn ensure_nk3_negotiation_accepts_forward_secure_suite() {
        let Nk3Fixture {
            client_caps,
            relay_caps,
            kem_id,
            sig_id,
            ..
        } = build_nk3_fixture();

        ensure_nk3_negotiation(
            client_caps.as_slice(),
            relay_caps.as_slice(),
            kem_id,
            sig_id,
        )
        .expect("nk3 negotiation succeeds");
    }

    #[test]
    fn ensure_nk3_negotiation_detects_downgrade() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );

        let err = ensure_nk3_negotiation(
            client_caps.as_slice(),
            relay_caps.as_slice(),
            defaults.kem_id,
            defaults.sig_id,
        )
        .expect_err("nk3 downgrade should surface");
        assert!(matches!(err, HarnessError::Downgrade { .. }));
    }

    #[test]
    fn compute_nk3_transcript_matches_parsed_hash() {
        let Nk3Fixture {
            client_state,
            relay_response,
            client_caps,
            kem_id,
            sig_id,
            ..
        } = build_nk3_fixture();

        let transcript = compute_nk3_transcript(
            &relay_response,
            &client_state.client_nonce,
            client_caps.as_slice(),
            kem_id,
            sig_id,
            client_state.resume_hash.as_deref(),
        )
        .expect("nk3 transcript");
        assert_eq!(transcript, relay_response.transcript_hash);
    }

    #[test]
    fn decapsulate_nk3_secrets_validates_artifacts() {
        let Nk3Fixture {
            client_state,
            relay_response,
            suite,
            client_caps,
            kem_id,
            sig_id,
            ..
        } = build_nk3_fixture();

        let transcript = compute_nk3_transcript(
            &relay_response,
            &client_state.client_nonce,
            client_caps.as_slice(),
            kem_id,
            sig_id,
            client_state.resume_hash.as_deref(),
        )
        .expect("nk3 transcript");

        let forward_secret = client_state
            .forward_kem_secret
            .as_ref()
            .expect("forward secret present");
        let forward_public = client_state
            .forward_kem_public
            .as_ref()
            .expect("forward public present");

        let (primary_shared, forward_shared) = decapsulate_nk3_secrets(
            suite,
            &client_state.client_kem_secret,
            forward_secret,
            forward_public.as_slice(),
            &relay_response,
            &transcript,
        )
        .expect("nk3 decapsulation");

        let recomputed_dual_mix = compute_dual_mix(
            primary_shared.as_bytes(),
            forward_shared.as_bytes(),
            &transcript,
        );
        assert_eq!(recomputed_dual_mix, relay_response.dual_mix);
    }

    #[test]
    fn assemble_relay_state_populates_core_fields() {
        let params = RuntimeParams::soranet_defaults();
        let parsed = ClientHelloParsed {
            client_nonce: [0x01; 32],
            handshake_suite: HandshakeSuite::Nk2Hybrid,
            kem_id: 5,
            sig_id: 7,
            client_ephemeral_public: [0x02; 32],
            client_static_public: Some([0x03; 32]),
            client_kem_public: vec![0x04, 0x05],
            forward_kem_public: None,
            forward_commitment: None,
            client_capabilities: vec![0x06],
            resume_hash: Some(vec![0x07]),
        };

        let ephemeral_secret = StaticSecret::from([0x22; 32]);
        let expected_ephemeral_public = X25519PublicKey::from(&ephemeral_secret).to_bytes();
        let static_secret = StaticSecret::from([0x33; 32]);
        let expected_static_public = X25519PublicKey::from(&static_secret).to_bytes();
        let noise = RelayNoiseState {
            nonce: [0xAA; 32],
            ephemeral_secret,
            ephemeral_public: expected_ephemeral_public,
            static_secret,
            static_public: expected_static_public,
        };

        let kem = RuntimeKemArtifacts {
            relay_public: vec![0x10, 0x11],
            relay_secret: Zeroizing::new(vec![0x20]),
            shared_secret: Zeroizing::new(vec![0x30]),
            ciphertext: vec![0x40, 0x41],
        };

        let warning_message = "test warning propagated".to_owned();
        let relay_hello = vec![0xDE, 0xAD];
        let transcript = [0xBB; 32];

        let state = assemble_relay_state(RelayStateInputs {
            parsed,
            params: &params,
            noise,
            kem,
            transcript,
            handshake_suite: HandshakeSuite::Nk2Hybrid,
            warnings: vec![CapabilityWarning {
                capability_type: 0x0101,
                message: warning_message.clone(),
            }],
            relay_hello: relay_hello.clone(),
        });

        assert_eq!(state.client_nonce, [0x01; 32]);
        assert_eq!(state.client_ephemeral_public, [0x02; 32]);
        assert_eq!(state.client_capabilities, vec![0x06]);
        assert_eq!(state.client_kem_public, vec![0x04, 0x05]);
        assert_eq!(state.resume_hash.as_deref(), Some(&[0x07][..]));
        assert_eq!(state.client_static_public, Some([0x03; 32]));
        assert_eq!(state.relay_nonce, [0xAA; 32]);
        assert_eq!(state.relay_ephemeral_public, expected_ephemeral_public);
        assert_eq!(state.relay_static_public, expected_static_public);
        assert_eq!(state.relay_capabilities, params.relay_capabilities);
        assert_eq!(state.descriptor_commit, params.descriptor_commit);
        assert_eq!(state.relay_hello, relay_hello);
        assert_eq!(state.transcript_hash, transcript);
        assert_eq!(state.kem_id, 5);
        assert_eq!(state.sig_id, 7);
        assert_eq!(state.handshake_suite, HandshakeSuite::Nk2Hybrid);
        assert_eq!(state.warnings.len(), 1);
        assert_eq!(state.warnings[0].capability_type, 0x0101);
        assert_eq!(state.warnings[0].message, warning_message);
        assert!(state.pending_session.is_none());
        assert!(state.transcript_confirm_primary.is_none());
        assert!(state.transcript_confirm_forward.is_none());
        assert!(state.forward_kem_public.is_none());
        assert!(!state.requires_client_finish);
        assert_eq!(&**state.relay_kem_shared, &[0x30]);
        assert_eq!(&**state.relay_kem_secret, &[0x20]);
        assert_eq!(state.kem_ciphertext, vec![0x40, 0x41]);
    }

    #[test]
    fn pqfs_relay_response_serializes_inputs_and_signatures() {
        let params = RuntimeParams::soranet_defaults();

        let noise = fixture_noise_state();
        let primary = fixture_kem_artifacts(
            &[0x01, 0x02, 0x03],
            &[0x04, 0x05],
            &[0x06, 0x07, 0x08, 0x09],
            &[0x0A, 0x0B, 0x0C, 0x0D],
        );
        let forward = fixture_kem_artifacts(
            &[0x21, 0x22, 0x23],
            &[0x24, 0x25],
            &[0x26, 0x27, 0x28, 0x29],
            &[0x2A, 0x2B, 0x2C, 0x2D],
        );

        let client_commit: &[u8] = b"unit-client-commit";
        let (primary_confirmation, forward_confirmation) =
            (vec![0x31, 0x32, 0x33, 0x34], vec![0x35, 0x36, 0x37, 0x38]);
        let transcript = [0x40; 32];
        let (forward_commitment, dual_mix) = (
            vec![0x41, 0x42, 0x43, 0x44],
            vec![0x50, 0x51, 0x52, 0x53, 0x54],
        );

        let inputs = PqfsRelayResponseInputs {
            client_commit,
            params: &params,
            noise: &noise,
            primary: &primary,
            forward: &forward,
            primary_confirmation: primary_confirmation.as_slice(),
            forward_confirmation: forward_confirmation.as_slice(),
            transcript: &transcript,
            forward_commitment: forward_commitment.as_slice(),
            dual_mix: dual_mix.as_slice(),
        };

        let response = build_pqfs_relay_response(&inputs).expect("build pqfs relay response");
        assert_eq!(response.len() % NOISE_PADDING_BLOCK, 0);

        let mut cursor = MessageCursor::new(&response);
        assert_eq!(
            cursor.read_u8().expect("response type"),
            PQFS_RELAY_RESPONSE_TYPE
        );
        let expected_segments: [(&str, &[u8]); 14] = [
            ("nonce segment", noise.nonce.as_ref()),
            ("ephemeral public", noise.ephemeral_public.as_ref()),
            ("static public", noise.static_public.as_ref()),
            ("relay capabilities", params.relay_capabilities),
            ("descriptor commit", params.descriptor_commit),
            ("primary relay public", primary.relay_public.as_slice()),
            ("primary ciphertext", primary.ciphertext.as_slice()),
            ("forward relay public", forward.relay_public.as_slice()),
            ("forward ciphertext", forward.ciphertext.as_slice()),
            ("primary confirmation", primary_confirmation.as_slice()),
            ("forward confirmation", forward_confirmation.as_slice()),
            ("transcript hash", transcript.as_ref()),
            ("forward commitment", forward_commitment.as_slice()),
            ("dual mix", dual_mix.as_slice()),
        ];

        for (label, expected) in expected_segments {
            assert_eq!(
                cursor.read_len_prefixed().expect(label),
                expected,
                "{label} mismatch"
            );
        }

        let body_len = response.len() - cursor.remaining_slice().len();
        let relay_body = &response[..body_len];

        let dilithium = cursor.read_len_prefixed().expect("dilithium signature");
        let expected_dilithium = expand_material(
            b"soranet.sig.dilithium.pqfs.relay",
            &[
                noise.static_secret.to_bytes().as_ref(),
                client_commit,
                relay_body,
                transcript.as_ref(),
            ],
            DILITHIUM3_SIGNATURE_LEN,
        );
        assert_eq!(dilithium, expected_dilithium.as_slice());

        let ed_signature = cursor.read_len_prefixed().expect("ed25519 signature");
        let expected_ed = derive_ed25519_signature(
            b"soranet.sig.ed25519.pqfs.relay",
            noise.static_secret.to_bytes().as_ref(),
            &[client_commit, relay_body, transcript.as_ref()],
        )
        .expect("derive ed25519 signature");
        assert_eq!(ed_signature, expected_ed.as_ref());

        let padding = cursor.remaining_slice();
        assert!(
            !padding.is_empty(),
            "noise padding should extend the response to the block size"
        );
        assert!(
            padding.iter().all(|&byte| byte == 0),
            "noise padding must be zeroed"
        );
    }

    #[test]
    fn parse_and_diff_capabilities_handles_required_flag() {
        let client = decode_hex("010100020101") // type 0x0101, required flag set, value 0x01
            .expect("client hex");
        let relay = Vec::new(); // relay omits the required capability
        let client_caps = parse_capabilities(&client).expect("parse client");
        let relay_caps = parse_capabilities(&relay).expect("parse relay");
        assert!(client_caps[0].required);
        let warnings = diff_capabilities(&client_caps, &relay_caps);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].capability_type, 0x0101);
        assert!(warnings[0].message.contains("0x0101"));
    }

    #[test]
    fn encode_capabilities_sets_required_flag_in_flags_byte() {
        let entries = vec![CapabilityTlv {
            ty: CAPABILITY_PQKEM,
            value: vec![0x01, 0x00],
            required: true,
        }];
        let encoded = encode_capabilities(&entries);
        let expected = decode_hex("010100020101").expect("expected hex");
        assert_eq!(encoded, expected);
    }

    #[test]
    fn transcript_hash_matches_reference() {
        let descriptor_commit = [0x11u8; 32];
        let client_nonce = [0x22u8; 32];
        let relay_nonce = [0x33u8; 32];
        let caps = [0u8; 8];
        let inputs = TranscriptInputs {
            descriptor_commit: &descriptor_commit,
            client_nonce: &client_nonce,
            relay_nonce: &relay_nonce,
            capability_bytes: &caps,
            kem_id: 1,
            sig_id: 1,
            handshake_suite: HandshakeSuite::Nk2Hybrid,
            resume_hash: None,
        };
        let hash = inputs.compute_hash();
        assert_eq!(hash.len(), 32);
    }

    #[test]
    fn suite_negotiation_prefers_highest_common_suite() {
        let client = suite_list_tlv(
            false,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
        );
        let relay = suite_list_tlv(false, &[HandshakeSuite::Nk3PqForwardSecure]);
        let client_caps = parse_capabilities(&client).expect("client");
        let relay_caps = parse_capabilities(&relay).expect("relay");
        let negotiation = negotiate_handshake_suite(&client_caps, &relay_caps).expect("negotiate");
        assert_eq!(negotiation.selected, HandshakeSuite::Nk3PqForwardSecure);
        assert!(negotiation.warnings.is_empty());
    }

    #[test]
    fn suite_negotiation_warns_on_downgrade() {
        let client = suite_list_tlv(
            true,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
        );
        let relay = suite_list_tlv(false, &[HandshakeSuite::Nk2Hybrid]);
        let client_caps = parse_capabilities(&client).expect("client");
        let relay_caps = parse_capabilities(&relay).expect("relay");
        let negotiation = negotiate_handshake_suite(&client_caps, &relay_caps).expect("negotiate");
        assert_eq!(negotiation.selected, HandshakeSuite::Nk2Hybrid);
        assert!(
            negotiation
                .warnings
                .iter()
                .any(|warn| warn.message.contains("client preferred")),
            "expected downgrade warning when preference not met"
        );
    }

    #[test]
    fn suite_negotiation_errors_without_overlap() {
        let client = suite_list_tlv(false, &[HandshakeSuite::Nk3PqForwardSecure]);
        let relay = suite_list_tlv(false, &[HandshakeSuite::Nk2Hybrid]);
        let client_caps = parse_capabilities(&client).expect("client");
        let relay_caps = parse_capabilities(&relay).expect("relay");
        let err = negotiate_handshake_suite(&client_caps, &relay_caps)
            .expect_err("expected downgrade error when suites do not overlap");
        match err {
            HarnessError::Downgrade { warnings, .. } => {
                assert!(
                    warnings
                        .iter()
                        .any(|warn| warn.message.contains("no overlapping handshake suite")),
                    "expected warning describing missing overlap"
                );
            }
            other => panic!("expected downgrade error, got {other:?}"),
        }
    }

    #[test]
    fn suite_negotiation_errors_when_relay_omits_capability() {
        let client = suite_list_tlv(
            false,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
        );
        let client_caps = parse_capabilities(&client).expect("client");
        let relay_caps = parse_capabilities(&[]).expect("relay");
        let err = negotiate_handshake_suite(&client_caps, &relay_caps)
            .expect_err("expected downgrade error when relay omits suite list");
        assert!(matches!(err, HarnessError::Downgrade { .. }));
    }

    #[test]
    fn salt_announcement_roundtrip() {
        let salt = [0x55u8; 32];
        let json = salt_announcement_json(&SaltAnnouncementParams {
            epoch_id: 7,
            previous_epoch: Some(6),
            valid_after: "2026-01-01T00:00:00Z",
            valid_until: "2026-01-02T00:00:00Z",
            blinded_cid_salt: &salt,
            emergency_rotation: false,
            notes: Some("test"),
            signature: None,
        })
        .expect("json");
        let value: Value = norito::json::from_str(&json).expect("json parse");
        assert_eq!(value["epoch_id"].as_u64(), Some(7));
        assert_eq!(value["previous_epoch"].as_u64(), Some(6));
        assert_eq!(
            value["blinded_cid_salt_hex"],
            Value::from(hex::encode(salt))
        );
        assert_eq!(value["signature"], Value::Null);
    }

    #[test]
    fn alarm_report_renders_expected_fields() {
        let json = alarm_report_json(&AlarmReport {
            relay_id: "relay",
            relay_role: 1,
            capability_type: 0x0101,
            reason_code: "MissingRequiredKEM",
            transcript_hash_hex: "deadbeef",
            observed_at_unix_ms: 1_785_120_000_000,
            circuit_id: 42,
            counter: 2,
            capability_value_hex: None,
            signature: Some("dilithium3:base64signature"),
            witness_signature: Some("ed25519:base64signature"),
        })
        .expect("alarm json");
        assert!(json.contains(r#""relay_id": "relay""#));
        assert!(json.contains("MissingRequiredKEM"));
        assert!(json.contains("dilithium3"));
        assert!(json.contains("ed25519"));
    }

    #[test]
    fn runtime_handshake_roundtrip_produces_matching_session_keys() {
        let params = RuntimeParams::soranet_defaults();
        let mut rng_client = StdRng::seed_from_u64(1);
        let mut rng_relay = StdRng::seed_from_u64(2);
        let client_keys = KeyPair::random();
        let relay_keys = KeyPair::random();

        let (client_hello, client_state) =
            build_client_hello(&params, &mut rng_client).expect("client hello");
        let (relay_hello, relay_state) =
            process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                .expect("relay hello");
        let (client_finish, client_secrets) = client_handle_relay_hello(
            client_state,
            &relay_hello,
            &client_keys,
            &params,
            &mut rng_client,
        )
        .expect("client finish");
        let relay_secrets = match client_finish {
            Some(frame) => {
                relay_finalize_handshake(relay_state, &frame, &relay_keys).expect("relay finish")
            }
            None => relay_finalize_handshake(relay_state, &[], &relay_keys).expect("relay finish"),
        };

        assert_eq!(
            client_secrets.handshake_suite,
            relay_secrets.handshake_suite
        );
        assert_eq!(client_secrets.session_key, relay_secrets.session_key);
        assert_eq!(
            client_secrets.transcript_hash,
            relay_secrets.transcript_hash
        );
    }

    #[test]
    fn build_client_hello_supports_nk2_preference() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            resume_hash: defaults.resume_hash,
        };
        let mut rng = StdRng::seed_from_u64(99);
        let (client_hello, client_state) =
            build_client_hello(&params, &mut rng).expect("nk2 client init");
        assert_eq!(
            client_hello.first().copied(),
            Some(HYBRID_CLIENT_HELLO_TYPE)
        );
        assert_eq!(client_state.handshake_suite, HandshakeSuite::Nk2Hybrid);
        assert!(client_state.forward_kem_public.is_none());
        assert!(client_state.forward_kem_secret.is_none());
    }

    #[test]
    fn process_client_hello_handles_nk2_suite() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            resume_hash: defaults.resume_hash,
        };

        let client_keys = KeyPair::random();
        let relay_keys = KeyPair::random();
        let mut rng_client = StdRng::seed_from_u64(100);
        let mut rng_relay = StdRng::seed_from_u64(200);

        let (client_hello, client_state) =
            build_client_hello(&params, &mut rng_client).expect("nk2 client");
        let (relay_hello, relay_state) =
            process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                .expect("nk2 relay");

        assert_eq!(relay_state.handshake_suite, HandshakeSuite::Nk2Hybrid);
        assert!(!relay_state.requires_client_finish);
        assert_eq!(
            relay_hello.first().copied(),
            Some(HYBRID_RELAY_RESPONSE_TYPE)
        );

        let (client_finish, client_secrets) = client_handle_relay_hello(
            client_state,
            &relay_hello,
            &client_keys,
            &params,
            &mut rng_client,
        )
        .expect("nk2 client handle");
        assert!(
            client_finish.is_none(),
            "nk2 handshake must not emit client finish"
        );
        let relay_secrets =
            relay_finalize_handshake(relay_state, &[], &relay_keys).expect("nk2 relay finalize");

        assert_eq!(client_secrets.handshake_suite, HandshakeSuite::Nk2Hybrid);
        assert_eq!(relay_secrets.handshake_suite, HandshakeSuite::Nk2Hybrid);
        assert_eq!(client_secrets.session_key, relay_secrets.session_key);
        assert_eq!(
            client_secrets.transcript_hash,
            relay_secrets.transcript_hash
        );
    }

    #[test]
    fn build_client_hello_supports_nk3_preference() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            resume_hash: defaults.resume_hash,
        };
        let mut rng = StdRng::seed_from_u64(1234);
        let (client_hello, client_state) =
            build_client_hello(&params, &mut rng).expect("nk3 client init");
        assert_eq!(client_hello.first().copied(), Some(PQFS_CLIENT_COMMIT_TYPE));
        assert_eq!(
            client_state.handshake_suite,
            HandshakeSuite::Nk3PqForwardSecure
        );
        assert!(client_state.forward_kem_public.is_some());
        assert!(client_state.forward_kem_secret.is_some());
    }

    #[test]
    fn process_client_hello_handles_nk3_suite() {
        let defaults = RuntimeParams::soranet_defaults();
        let client_caps = capabilities_with_suites(
            defaults.client_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let relay_caps = capabilities_with_suites(
            defaults.relay_capabilities,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: client_caps.as_slice(),
            relay_capabilities: relay_caps.as_slice(),
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            resume_hash: defaults.resume_hash,
        };

        let client_keys = KeyPair::random();
        let relay_keys = KeyPair::random();
        let mut rng_client = StdRng::seed_from_u64(2024);
        let mut rng_relay = StdRng::seed_from_u64(4048);

        let (client_hello, client_state) =
            build_client_hello(&params, &mut rng_client).expect("nk3 client");
        let (relay_hello, relay_state) =
            process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                .expect("nk3 relay");

        assert_eq!(
            relay_state.handshake_suite,
            HandshakeSuite::Nk3PqForwardSecure
        );
        assert!(!relay_state.requires_client_finish);
        assert_eq!(relay_hello.first().copied(), Some(PQFS_RELAY_RESPONSE_TYPE));

        let profile = kem_profile(params.kem_id).expect("kem profile");
        parse_pqfs_relay_response(&relay_hello, params.descriptor_commit, profile.suite())
            .expect("parse nk3 response");

        let (client_finish, client_secrets) = client_handle_relay_hello(
            client_state,
            &relay_hello,
            &client_keys,
            &params,
            &mut rng_client,
        )
        .expect("nk3 client handle");
        assert!(
            client_finish.is_none(),
            "nk3 handshake must not emit client finish"
        );
        let relay_secrets =
            relay_finalize_handshake(relay_state, &[], &relay_keys).expect("nk3 relay finalize");

        assert_eq!(
            client_secrets.handshake_suite,
            HandshakeSuite::Nk3PqForwardSecure
        );
        assert_eq!(
            relay_secrets.handshake_suite,
            HandshakeSuite::Nk3PqForwardSecure
        );
        assert_eq!(client_secrets.session_key, relay_secrets.session_key);
        assert_eq!(
            client_secrets.transcript_hash,
            relay_secrets.transcript_hash
        );
    }

    #[test]
    fn runtime_handshake_roundtrip_supports_mlkem1024() {
        let mut params = RuntimeParams::soranet_defaults();
        params.kem_id = 2;
        let mut rng_client = StdRng::seed_from_u64(11);
        let mut rng_relay = StdRng::seed_from_u64(12);
        let client_keys = KeyPair::random();
        let relay_keys = KeyPair::random();

        let (client_hello, client_state) =
            build_client_hello(&params, &mut rng_client).expect("client hello");
        let (relay_hello, relay_state) =
            process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay)
                .expect("relay hello");
        let (client_finish, client_secrets) = client_handle_relay_hello(
            client_state,
            &relay_hello,
            &client_keys,
            &params,
            &mut rng_client,
        )
        .expect("client finish");
        let relay_secrets = match client_finish {
            Some(frame) => {
                relay_finalize_handshake(relay_state, &frame, &relay_keys).expect("relay")
            }
            None => relay_finalize_handshake(relay_state, &[], &relay_keys).expect("relay"),
        };

        assert_eq!(
            client_secrets.handshake_suite,
            relay_secrets.handshake_suite
        );
        assert_eq!(client_secrets.session_key, relay_secrets.session_key);
        assert_eq!(
            client_secrets.transcript_hash,
            relay_secrets.transcript_hash
        );
    }

    #[test]
    fn build_client_hello_rejects_unknown_kem() {
        let mut params = RuntimeParams::soranet_defaults();
        params.kem_id = 0xFF;
        let mut rng = StdRng::seed_from_u64(21);
        let err = match build_client_hello(&params, &mut rng) {
            Ok(_) => panic!("expected invalid KEM id to produce error"),
            Err(err) => err,
        };
        match err {
            HarnessError::Validation(message) => {
                assert!(message.contains("ML-KEM"), "unexpected message: {message}");
            }
            other => panic!("expected validation error, got {other:?}"),
        }
    }

    #[test]
    fn process_client_hello_errors_on_missing_capability() {
        let defaults = RuntimeParams::soranet_defaults();
        let bad_relay_caps = hex_literal::hex!(
            "0102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f02010001010202000200047f12000412345678"
        );
        let params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: defaults.client_capabilities,
            relay_capabilities: &bad_relay_caps,
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            resume_hash: defaults.resume_hash,
        };

        let mut rng_client = StdRng::seed_from_u64(3);
        let (client_hello, _client_state) =
            build_client_hello(&defaults, &mut rng_client).expect("client hello");
        let relay_keys = KeyPair::random();
        let mut rng_relay = StdRng::seed_from_u64(4);

        match process_client_hello(&client_hello, &params, &relay_keys, &mut rng_relay) {
            Err(HarnessError::Downgrade {
                warnings,
                telemetry,
            }) => {
                assert!(!warnings.is_empty(), "expected downgrade warnings");
                assert!(
                    telemetry.is_some(),
                    "downgrade should emit telemetry payload"
                );
            }
            Err(err) => panic!("expected downgrade error, got {err:?}"),
            Ok(_) => panic!("expected downgrade error, got Ok"),
        }
    }

    #[test]
    fn process_client_hello_reports_constant_rate_warning() {
        let mut client_params = RuntimeParams::soranet_defaults();
        let mut client_entries =
            parse_capabilities(client_params.client_capabilities).expect("parse defaults");
        client_entries.push(CapabilityTlv {
            ty: CAPABILITY_CONSTANT_RATE,
            value: vec![0x01, 0x01],
            required: true,
        });
        client_entries.sort_by_key(|cap| cap.ty);
        let client_cap_bytes = encode_capabilities(&client_entries);
        client_params.client_capabilities = &client_cap_bytes;
        let mut expected_relay_entries =
            parse_capabilities(client_params.relay_capabilities).expect("parse relay defaults");
        expected_relay_entries.push(CapabilityTlv {
            ty: CAPABILITY_CONSTANT_RATE,
            value: vec![0x01, 0x01],
            required: true,
        });
        expected_relay_entries.sort_by_key(|cap| cap.ty);
        let expected_relay_caps = encode_capabilities(&expected_relay_entries);
        client_params.relay_capabilities = &expected_relay_caps;

        let mut rng_client = StdRng::seed_from_u64(31);
        let (client_hello, _client_state) =
            build_client_hello(&client_params, &mut rng_client).expect("client hello");

        let relay_keys = KeyPair::random();
        let mut rng_relay = StdRng::seed_from_u64(32);
        let relay_params = RuntimeParams::soranet_defaults();

        match process_client_hello(&client_hello, &relay_params, &relay_keys, &mut rng_relay) {
            Err(HarnessError::Downgrade { warnings, .. }) => {
                assert!(
                    warnings
                        .iter()
                        .any(|warn| warn.capability_type == CAPABILITY_CONSTANT_RATE
                            && warn.message.contains("snnet.constant_rate")),
                    "expected downgrade warning mentioning snnet.constant_rate: {warnings:?}"
                );
            }
            Err(err) => panic!("expected downgrade error, got {err:?}"),
            Ok(_) => panic!("expected downgrade when constant-rate is missing"),
        }
    }

    #[test]
    fn client_handle_relay_hello_rejects_descriptor_commit_mismatch() {
        let defaults = RuntimeParams::soranet_defaults();
        let mut rng_client = StdRng::seed_from_u64(5);
        let (client_hello, client_state) =
            build_client_hello(&defaults, &mut rng_client).expect("client hello");

        let mut altered_descriptor = *defaults
            .descriptor_commit
            .first()
            .expect("non-empty descriptor");
        altered_descriptor ^= 0xFF;
        let mut bad_descriptor = defaults.descriptor_commit.to_vec();
        bad_descriptor[0] = altered_descriptor;

        let bad_params = RuntimeParams {
            descriptor_commit: &bad_descriptor,
            client_capabilities: defaults.client_capabilities,
            relay_capabilities: defaults.relay_capabilities,
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            resume_hash: defaults.resume_hash,
        };

        let relay_keys = KeyPair::random();
        let mut rng_relay = StdRng::seed_from_u64(6);
        let (relay_hello, _relay_state) =
            process_client_hello(&client_hello, &bad_params, &relay_keys, &mut rng_relay)
                .expect("relay hello");

        let client_keys = KeyPair::random();

        match client_handle_relay_hello(
            client_state,
            &relay_hello,
            &client_keys,
            &defaults,
            &mut rng_client,
        ) {
            Err(HarnessError::Validation(message)) => {
                assert!(
                    message.contains("descriptor"),
                    "expected descriptor mismatch message, got {message}"
                );
            }
            Err(err) => panic!("expected descriptor mismatch, got {err:?}"),
            Ok(_) => panic!("expected descriptor mismatch, got Ok"),
        }
    }

    #[test]
    fn process_client_hello_rejects_resume_hash_mismatch() {
        let defaults = RuntimeParams::soranet_defaults();
        let resume_a =
            hex_literal::hex!("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff");
        let resume_b =
            hex_literal::hex!("ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100");

        let client_params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: defaults.client_capabilities,
            relay_capabilities: defaults.relay_capabilities,
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            resume_hash: Some(&resume_a),
        };
        let mismatched_params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: defaults.client_capabilities,
            relay_capabilities: defaults.relay_capabilities,
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            resume_hash: Some(&resume_b),
        };
        let absent_params = RuntimeParams {
            descriptor_commit: defaults.descriptor_commit,
            client_capabilities: defaults.client_capabilities,
            relay_capabilities: defaults.relay_capabilities,
            kem_id: defaults.kem_id,
            sig_id: defaults.sig_id,
            resume_hash: None,
        };

        let mut rng_client = StdRng::seed_from_u64(7);
        let (client_hello, _client_state) =
            build_client_hello(&client_params, &mut rng_client).expect("client hello");
        let relay_keys = KeyPair::random();

        let mut rng_relay = StdRng::seed_from_u64(8);
        match process_client_hello(
            &client_hello,
            &mismatched_params,
            &relay_keys,
            &mut rng_relay,
        ) {
            Err(HarnessError::Validation(message)) => {
                assert!(
                    message.contains("resume hash mismatch"),
                    "unexpected message: {message}"
                );
            }
            Err(err) => panic!("expected resume mismatch, got {err:?}"),
            Ok(_) => panic!("expected resume mismatch, got Ok"),
        }

        let mut rng_relay = StdRng::seed_from_u64(9);
        match process_client_hello(&client_hello, &absent_params, &relay_keys, &mut rng_relay) {
            Err(HarnessError::Validation(message)) => {
                assert!(
                    message.contains("unexpected resume hash"),
                    "unexpected message: {message}"
                );
            }
            Err(err) => panic!("expected unexpected resume hash error, got {err:?}"),
            Ok(_) => panic!("expected unexpected resume hash error, got Ok"),
        }
    }

    #[test]
    fn simulate_handshake_produces_transcript_hash() {
        let client_caps = DEFAULT_CLIENT_CAPABILITIES.to_vec();
        let relay_caps = DEFAULT_RELAY_CAPABILITIES.to_vec();
        let descriptor_commit =
            decode_hex("76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f")
                .expect("descriptor");
        let client_nonce =
            decode_hex("2c1f64028dbe42410d1921cd9a316bed4f8f5b52ffb62b4dcaf149048393ca8a")
                .expect("client nonce");
        let relay_nonce =
            decode_hex("d5f4f2f9c2b1a39e88bbd3c0a4f9e178d93e7bfacaf0c3e872b712f4a341c9de")
                .expect("relay nonce");
        let (client_static_sk, relay_static_sk) = sample_static_keys();

        let result = simulate_handshake(&SimulationParams {
            client_capabilities: &client_caps,
            relay_capabilities: &relay_caps,
            client_static_sk: &client_static_sk,
            relay_static_sk: &relay_static_sk,
            resume_hash: None,
            descriptor_commit: &descriptor_commit,
            client_nonce: &client_nonce,
            relay_nonce: &relay_nonce,
            kem_id: 1,
            sig_id: 1,
        })
        .expect("simulate");

        let expected = TranscriptInputs {
            descriptor_commit: &descriptor_commit,
            client_nonce: &client_nonce,
            relay_nonce: &relay_nonce,
            capability_bytes: &client_caps,
            kem_id: 1,
            sig_id: 1,
            handshake_suite: HandshakeSuite::Nk2Hybrid,
            resume_hash: None,
        }
        .compute_hash();
        assert_eq!(result.transcript_hash, expected);
        assert_eq!(result.handshake_suite, HandshakeSuite::Nk2Hybrid);
        assert!(result.warnings.is_empty());
        assert_eq!(result.telemetry_payloads.len(), 1);
        assert_eq!(result.handshake_steps.len(), 2);
        assert_eq!(result.handshake_steps[0].note, STEP_NOTE_HYBRID_INIT);
        assert_eq!(result.handshake_steps[1].note, STEP_NOTE_HYBRID_RESPONSE);
    }

    #[test]
    fn simulate_handshake_negotiates_nk2_hybrid_suite() {
        let client_caps = capabilities_with_suites(
            &DEFAULT_CLIENT_CAPABILITIES,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let relay_caps = capabilities_with_suites(
            &DEFAULT_RELAY_CAPABILITIES,
            &[HandshakeSuite::Nk2Hybrid],
            false,
        );
        let descriptor_commit = DEFAULT_DESCRIPTOR_COMMIT.to_vec();
        let client_nonce =
            decode_hex("1f2e3d4c5b6a79888796a5b4c3d2e1f00112233445566778899aabbccddeeff0")
                .expect("client nonce");
        let relay_nonce =
            decode_hex("2b64a7e5c1d3f4b2a9c8d7e6f5a4132233445566778899aabbccddeeff001122")
                .expect("relay nonce");
        let (client_static_sk, relay_static_sk) = sample_static_keys();

        let result = simulate_handshake(&SimulationParams {
            client_capabilities: &client_caps,
            relay_capabilities: &relay_caps,
            client_static_sk: &client_static_sk,
            relay_static_sk: &relay_static_sk,
            resume_hash: None,
            descriptor_commit: &descriptor_commit,
            client_nonce: &client_nonce,
            relay_nonce: &relay_nonce,
            kem_id: 1,
            sig_id: 1,
        })
        .expect("simulate");

        assert_eq!(result.handshake_suite, HandshakeSuite::Nk2Hybrid);
        assert!(result.warnings.is_empty());
        assert_eq!(result.handshake_steps.len(), 2);
        assert_eq!(result.handshake_steps[0].note, STEP_NOTE_HYBRID_INIT);
        assert_eq!(result.handshake_steps[1].note, STEP_NOTE_HYBRID_RESPONSE);
        assert_eq!(result.telemetry_payloads.len(), 1);
    }

    #[test]
    fn simulate_handshake_negotiates_nk3_forward_secure_suite() {
        let client_caps = capabilities_with_suites(
            &DEFAULT_CLIENT_CAPABILITIES,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let relay_caps = capabilities_with_suites(
            &DEFAULT_RELAY_CAPABILITIES,
            &[
                HandshakeSuite::Nk3PqForwardSecure,
                HandshakeSuite::Nk2Hybrid,
            ],
            false,
        );
        let descriptor_commit = DEFAULT_DESCRIPTOR_COMMIT.to_vec();
        let client_nonce =
            decode_hex("3c2b1a09180706050403020100112233445566778899aabbccddeeff00112233")
                .expect("client nonce");
        let relay_nonce =
            decode_hex("445566778899aabbccddeeff00112233445566778899aabbccddeeff10213254")
                .expect("relay nonce");
        let (client_static_sk, relay_static_sk) = sample_static_keys();

        let result = simulate_handshake(&SimulationParams {
            client_capabilities: &client_caps,
            relay_capabilities: &relay_caps,
            client_static_sk: &client_static_sk,
            relay_static_sk: &relay_static_sk,
            resume_hash: None,
            descriptor_commit: &descriptor_commit,
            client_nonce: &client_nonce,
            relay_nonce: &relay_nonce,
            kem_id: 1,
            sig_id: 1,
        })
        .expect("simulate");

        assert_eq!(result.handshake_suite, HandshakeSuite::Nk3PqForwardSecure);
        assert!(result.warnings.is_empty());
        assert_eq!(result.handshake_steps.len(), 2);
        assert_eq!(result.handshake_steps[0].note, STEP_NOTE_PQFS_COMMIT);
        assert_eq!(result.handshake_steps[1].note, STEP_NOTE_PQFS_RESPONSE);
        assert_eq!(result.telemetry_payloads.len(), 1);
    }

    #[test]
    fn simulate_handshake_surfaces_warnings() {
        let client_caps = decode_hex(
            "0101000201010102000201010104000282030202000200047f100004deadbeef7f110004cafebabe",
        )
        .expect("client hex");
        let relay_caps = decode_hex(
            "0102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f01040002820302010001010202000200047f1300040badc0de",
        )
        .expect("relay hex");
        let descriptor_commit =
            decode_hex("76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f")
                .expect("descriptor");
        let client_nonce =
            decode_hex("1f2e3d4c5b6a79888796a5b4c3d2e1f00112233445566778899aabbccddeeff0")
                .expect("client nonce");
        let relay_nonce =
            decode_hex("2b64a7e5c1d3f4b2a9c8d7e6f5a4132233445566778899aabbccddeeff001122")
                .expect("relay nonce");
        let (client_static_sk, relay_static_sk) = sample_static_keys();

        let result = simulate_handshake(&SimulationParams {
            client_capabilities: &client_caps,
            relay_capabilities: &relay_caps,
            client_static_sk: &client_static_sk,
            relay_static_sk: &relay_static_sk,
            resume_hash: None,
            descriptor_commit: &descriptor_commit,
            client_nonce: &client_nonce,
            relay_nonce: &relay_nonce,
            kem_id: 1,
            sig_id: 1,
        })
        .expect("simulate");

        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings[0].capability_type, 0x0101);
        assert!(result.warnings[0].message.contains("snnet.pqkem"));
        assert_eq!(result.telemetry_payloads.len(), 1);
    }

    #[test]
    fn simulation_report_json_renders_expected_fields() {
        let client_caps = decode_hex(
            "0101000201010102000201010104000282030202000200047f100004deadbeef7f110004cafebabe",
        )
        .expect("client hex");
        let relay_caps = decode_hex(
            "0101000201010102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f01040002820302010001010202000200047f12000412345678",
        )
        .expect("relay hex");
        let descriptor_commit =
            decode_hex("76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f")
                .expect("descriptor");
        let client_nonce =
            decode_hex("2c1f64028dbe42410d1921cd9a316bed4f8f5b52ffb62b4dcaf149048393ca8a")
                .expect("client nonce");
        let relay_nonce =
            decode_hex("d5f4f2f9c2b1a39e88bbd3c0a4f9e178d93e7bfacaf0c3e872b712f4a341c9de")
                .expect("relay nonce");
        let (client_static_sk, relay_static_sk) = sample_static_keys();

        let result = simulate_handshake(&SimulationParams {
            client_capabilities: &client_caps,
            relay_capabilities: &relay_caps,
            client_static_sk: &client_static_sk,
            relay_static_sk: &relay_static_sk,
            resume_hash: None,
            descriptor_commit: &descriptor_commit,
            client_nonce: &client_nonce,
            relay_nonce: &relay_nonce,
            kem_id: 1,
            sig_id: 1,
        })
        .expect("simulate");

        let json = simulation_report_json(&result, None).expect("json");
        assert!(json.contains(r#""transcript_hash_hex": "fd92a86d""#));
        assert!(json.contains(r#""handshake_suite": "nk2.hybrid""#));
        assert!(json.contains(r#""client_capabilities""#));
        assert!(json.contains(r#""relay_capabilities""#));
        assert!(json.contains(r#""handshake_steps""#));
    }

    #[test]
    fn simulation_report_json_filters_warnings() {
        let client_caps = decode_hex(
            "0101000201010102000201010104000282030202000200047f100004deadbeef7f110004cafebabe",
        )
        .expect("client hex");
        let relay_caps = decode_hex(
            "0102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f01040002820302010001010202000200047f1300040badc0de",
        )
        .expect("relay hex");
        let descriptor_commit =
            decode_hex("76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f")
                .expect("descriptor");
        let client_nonce =
            decode_hex("1f2e3d4c5b6a79888796a5b4c3d2e1f00112233445566778899aabbccddeeff0")
                .expect("client nonce");
        let relay_nonce =
            decode_hex("2b64a7e5c1d3f4b2a9c8d7e6f5a4132233445566778899aabbccddeeff001122")
                .expect("relay nonce");
        let (client_static_sk, relay_static_sk) = sample_static_keys();

        let result = simulate_handshake(&SimulationParams {
            client_capabilities: &client_caps,
            relay_capabilities: &relay_caps,
            client_static_sk: &client_static_sk,
            relay_static_sk: &relay_static_sk,
            resume_hash: None,
            descriptor_commit: &descriptor_commit,
            client_nonce: &client_nonce,
            relay_nonce: &relay_nonce,
            kem_id: 1,
            sig_id: 1,
        })
        .expect("simulate");

        let json = simulation_report_json(&result, Some(&[0x0202])).expect("json");
        let value: Value = norito::json::from_str(&json).expect("json parse");
        assert!(value["warnings"].as_array().unwrap().is_empty());
    }

    #[test]
    fn suite_key_schedule_uses_distinct_domains() {
        let transcript = [0xAA; 32];
        let primary = [0x11; 32];
        let forward = [0x22; 32];
        let (nk2_session, nk2_confirm) = derive_session_key_and_confirmation(SessionKeyInputs {
            suite: HandshakeSuite::Nk2Hybrid,
            transcript_hash: &transcript,
            primary_shared: &primary,
            forward_shared: None,
        })
        .expect("nk2 schedule");

        let (nk3_session, nk3_confirm) = derive_session_key_and_confirmation(SessionKeyInputs {
            suite: HandshakeSuite::Nk3PqForwardSecure,
            transcript_hash: &transcript,
            primary_shared: &primary,
            forward_shared: Some(&forward),
        })
        .expect("nk3 schedule");

        assert_ne!(nk2_session, nk3_session);
        assert_ne!(nk2_confirm, nk3_confirm);
    }

    #[test]
    fn nk3_key_schedule_requires_forward_secret() {
        let transcript = [0xAB; 32];
        let primary = [0xCD; 32];

        let err = derive_session_key_and_confirmation(SessionKeyInputs {
            suite: HandshakeSuite::Nk3PqForwardSecure,
            transcript_hash: &transcript,
            primary_shared: &primary,
            forward_shared: None,
        })
        .expect_err("nk3 schedule without forward secret must error");

        match err {
            HarnessError::NotImplemented(message) => {
                assert!(
                    message.contains("forward-secure"),
                    "unexpected error message: {message}"
                );
            }
            other => panic!("unexpected error {other:?}"),
        }
    }
}
