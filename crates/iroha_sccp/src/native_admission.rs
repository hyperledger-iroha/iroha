//! Typed, protocol-native SCCP inbound admission.
//!
//! The first-release wire format has one closed proof variant per supported source-chain verifier.
//! Consensus evidence is never routed through an opaque byte field inside this envelope: every
//! native DTO is decoded as its concrete Rust/Norito type before verifier dispatch.
use super::{
    BscNativeSourceError, BscNativeSourceProofV1, EthereumNativeSourceErrorV1,
    EthereumNativeSourceProofV1, H256, SccpPayloadV1, SccpSolanaAgaveSourceProofV1,
    SolanaNativeSourceErrorV1, TonNativeSourceError, TonNativeSourceProofV1, TronNativeSourceError,
    TronNativeSourceProofV1, bsc_native_anchor_block_number, canonical_sccp_payload_bytes,
    payload_hash, sccp_lane_id_hash_v1, sccp_lane_source_event_digest_v1, sccp_message_id,
    sccp_message_source_domain, sccp_message_target_domain, sccp_source_identity_hash_v1,
    verify_bsc_native_source, verify_ethereum_native_source_proof_v1,
    verify_sccp_payload_structure, verify_sccp_solana_agave_source_v1, verify_ton_native_source,
    verify_tron_native_source,
};
use alloc::{boxed::Box, vec::Vec};
use core::fmt;
use iroha_data_model::bridge::{
    BridgeNativeProofBackendV1, BridgeNativeProtocolProofV1, SccpInboundMessageKeyV1, SccpLaneIdV1,
    SccpNativeTrustAnchorV1, SccpNetworkV1, SccpSourceIdentityV1,
};
/// Maximum canonical Norito size of a native source envelope or inbound proof.
///
/// This bound is checked before decoding, after decoding by canonical
/// re-encoding, and when producing a data-model bridge container.
pub const SCCP_NATIVE_ADMISSION_MAX_ENCODED_BYTES_V1: usize = 16 * 1024 * 1024;
/// Maximum canonical padded-base64 length of one native admission proof.
///
/// Public HTTP adapters should enforce this bound before allocating a decoded
/// byte vector. It is the exact `4 * ceil(binary_max / 3)` expansion bound.
pub const SCCP_NATIVE_ADMISSION_MAX_BASE64_BYTES_V1: usize =
    4 * SCCP_NATIVE_ADMISSION_MAX_ENCODED_BYTES_V1.div_ceil(3);
/// Maximum UTF-8 byte length accepted by native admission JSON decoders.
///
/// Hex-rendered binary fields need approximately twice their binary size; the additional headroom
/// covers field names and JSON punctuation without making the allocation bound input-dependent.
pub const SCCP_NATIVE_ADMISSION_MAX_JSON_BYTES_V1: usize = 40 * 1024 * 1024;
/// Maximum canonical SCCP application-payload size admitted by the wrapper.
pub const SCCP_NATIVE_ADMISSION_MAX_PAYLOAD_BYTES_V1: usize =
    iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGE_MAX_PAYLOAD_BYTES_V1;
const SCCP_NATIVE_ADMISSION_MAX_ENCODED_BYTES_U64_V1: u64 = 16 * 1024 * 1024;
const NORITO_COMPRESSION_OFFSET: usize = 4 + 1 + 1 + 16;
const NORITO_LENGTH_OFFSET: usize = NORITO_COMPRESSION_OFFSET + 1;
/// One closed, typed protocol-native source proof.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[norito(tag = "backend", content = "proof", rename_all = "snake_case")]
pub enum SccpNativeSourceProofV1 {
    /// Ethereum beacon-light-client and execution-MPT proof.
    EthereumBeacon(Box<EthereumNativeSourceProofV1>),
    /// BNB Smart Chain Parlia finality and execution-MPT proof.
    BscParlia(Box<BscNativeSourceProofV1>),
    /// Solana testnet recursive Agave rooted-bank and instruction proof.
    SolanaAgave(Box<SccpSolanaAgaveSourceProofV1>),
    /// TRON `DPoS` replay and transaction-inclusion proof.
    TronDpos(Box<TronNativeSourceProofV1>),
    /// TON masterchain-finality, shard-state, and source-message proof.
    TonMasterchain(Box<TonNativeSourceProofV1>),
}
impl SccpNativeSourceProofV1 {
    /// Return the closed bridge backend selected by this typed variant.
    #[must_use]
    pub const fn backend(&self) -> BridgeNativeProofBackendV1 {
        match self {
            Self::EthereumBeacon(_) => BridgeNativeProofBackendV1::EthereumBeacon,
            Self::BscParlia(_) => BridgeNativeProofBackendV1::BscParlia,
            Self::SolanaAgave(_) => BridgeNativeProofBackendV1::SolanaAgave,
            Self::TronDpos(_) => BridgeNativeProofBackendV1::TronDpos,
            Self::TonMasterchain(_) => BridgeNativeProofBackendV1::TonMasterchain,
        }
    }
    fn embedded_source_network(&self) -> SccpNetworkV1 {
        match self {
            Self::EthereumBeacon(proof) => proof.source_identity.lane.source,
            Self::BscParlia(proof) => proof.finality.anchor.network,
            Self::SolanaAgave(proof) => proof.anchor.network,
            Self::TronDpos(proof) => proof.finality.anchor.network,
            Self::TonMasterchain(proof) => proof.finality.anchor.network,
        }
    }
}
/// Exact-lane native proof statement authenticated by SCCP governance.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct SccpNativeSourceProofEnvelopeV1 {
    /// Envelope schema version. V1 accepts exactly `1`.
    pub version: u8,
    /// Exact external-source to SORA-target network lane.
    pub lane: SccpLaneIdV1,
    /// Governed canonical hash of the lane's full source identity.
    #[norito(with = "crate::json_utils::hex32")]
    pub source_identity_hash: H256,
    /// Governed, family-tagged native checkpoint commitment.
    pub trust_anchor: SccpNativeTrustAnchorV1,
    /// Canonical identifier derived from the SCCP application payload.
    #[norito(with = "crate::json_utils::hex32")]
    pub message_id: H256,
    /// Canonical hash of the SCCP application payload bytes.
    #[norito(with = "crate::json_utils::hex32")]
    pub payload_hash: H256,
    /// Canonical exact-lane source-event digest.
    #[norito(with = "crate::json_utils::hex32")]
    pub source_event_digest: H256,
    /// Native height and block hash claimed by this self-contained proof.
    ///
    /// Admission accepts the claim only when the selected full native verifier
    /// independently reproduces both fields exactly.
    pub source_finality: SccpNativeFinalityPointV1,
    /// Concrete native consensus and event-inclusion proof.
    pub proof: SccpNativeSourceProofV1,
}
/// Compact native SCCP inbound proof.
///
/// Native source events directly authenticate the message statement, so this
/// wrapper carries no synthetic commitment root or unrelated Merkle branch.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct SccpNativeInboundMessageProofV1 {
    /// Wrapper schema version. V1 accepts exactly `1`.
    pub version: u8,
    /// Canonical SCCP application payload.
    pub payload: SccpPayloadV1,
    /// Typed native source proof and exact message statement.
    pub source: SccpNativeSourceProofEnvelopeV1,
}
/// Normalized source-finality point containing the native event.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct SccpNativeFinalityPointV1 {
    /// Native block number, slot, or shard sequence number.
    #[norito(with = "crate::json_utils::u64_string")]
    pub height: u64,
    /// Native hash of the block or bank containing the authenticated event.
    #[norito(with = "crate::json_utils::hex32")]
    pub block_hash: H256,
}
/// Chain-independent result of complete native SCCP admission.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
pub struct ValidatedSccpNativeInboundMessageV1 {
    /// Exact-lane replay key.
    pub message_key: SccpInboundMessageKeyV1,
    /// Canonical exact-lane hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub lane_hash: H256,
    /// Governed canonical source-identity hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub source_identity_hash: H256,
    /// Governed native checkpoint commitment used by the verifier.
    pub trust_anchor: SccpNativeTrustAnchorV1,
    /// Canonical SCCP payload hash.
    #[norito(with = "crate::json_utils::hex32")]
    pub payload_hash: H256,
    /// Canonical exact-lane event digest authenticated by the native proof.
    #[norito(with = "crate::json_utils::hex32")]
    pub source_event_digest: H256,
    /// Native finality point containing the source event.
    pub source_finality: SccpNativeFinalityPointV1,
    /// Authenticated consensus-progress coordinate comparable to governed
    /// native trust-anchor checkpoint heights.
    ///
    /// Ethereum uses the finalized beacon slot; BSC and TRON use the finalized
    /// native block height. This is deliberately distinct from
    /// `source_finality.height`, which is the event-containing execution block.
    #[norito(with = "crate::json_utils::u64_string")]
    pub anchor_interval_height: u64,
}
/// Fail-closed native admission error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SccpNativeAdmissionErrorV1 {
    /// A wrapper or envelope version was not V1.
    UnsupportedVersion(&'static str),
    /// The encoded binary or JSON input was empty.
    EmptyEncoding,
    /// The encoded proof exceeded the deterministic byte bound.
    EncodedSize {
        /// Number of encoded bytes supplied or declared by the caller.
        actual: usize,
        /// Maximum encoded byte length accepted by V1.
        maximum: usize,
    },
    /// Binary input was not one canonical, trailing-free Norito value.
    InvalidNoritoEncoding,
    /// JSON input was malformed, trailing, or used an unknown field/variant.
    InvalidJsonEncoding,
    /// The lane was not an exact external-source to SORA-target lane.
    InvalidLane,
    /// The typed proof, network, trust anchor, or source family did not match.
    BackendMismatch,
    /// The exact external-to-SORA direction is deliberately unavailable in V1.
    SourceDirectionUnavailable {
        /// Exact source profile whose inbound path is disabled.
        source: SccpNetworkV1,
    },
    /// The governed source identity was malformed or selected a different lane.
    InvalidSourceIdentity,
    /// The governed source-identity commitment did not match.
    SourceIdentityHashMismatch,
    /// The governed trust-anchor commitment did not match the envelope/proof.
    TrustAnchorMismatch,
    /// The SCCP payload was malformed or exceeded its deterministic bound.
    InvalidPayload,
    /// Payload source or target domains did not match the exact lane.
    PayloadLaneMismatch,
    /// The envelope did not carry the payload-derived message identifier.
    MessageIdMismatch,
    /// The envelope did not carry the canonical payload hash.
    PayloadHashMismatch,
    /// The envelope did not carry the canonical exact-lane event digest.
    SourceEventDigestMismatch,
    /// Two non-interchangeable hash roles reused one value or a zero sentinel.
    HashRoleCollision,
    /// Native Ethereum verification failed.
    Ethereum(EthereumNativeSourceErrorV1),
    /// Native BNB Smart Chain verification failed.
    Bsc(BscNativeSourceError),
    /// Native Solana testnet recursive verification failed.
    Solana(SolanaNativeSourceErrorV1),
    /// Native TRON verification failed.
    Tron(TronNativeSourceError),
    /// Native TON verification failed.
    Ton(TonNativeSourceError),
    /// A native verifier returned fields inconsistent with the admitted statement.
    NormalizedResultMismatch(&'static str),
}
impl fmt::Display for SccpNativeAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(role) => write!(formatter, "unsupported {role} version"),
            Self::EmptyEncoding => formatter.write_str("encoded native proof is empty"),
            Self::EncodedSize { actual, maximum } => {
                write!(
                    formatter,
                    "encoded native proof size {actual} exceeds {maximum}"
                )
            }
            Self::InvalidNoritoEncoding => {
                formatter.write_str("invalid canonical Norito native admission proof")
            }
            Self::InvalidJsonEncoding => {
                formatter.write_str("invalid strict JSON native admission proof")
            }
            Self::InvalidLane => formatter.write_str("invalid native inbound SCCP lane"),
            Self::BackendMismatch => {
                formatter.write_str("native proof backend or network mismatch")
            }
            Self::SourceDirectionUnavailable { source } => write!(
                formatter,
                "{} cannot be an SCCP native inbound source in V1",
                source.profile_key()
            ),
            Self::InvalidSourceIdentity => {
                formatter.write_str("invalid governed native source identity")
            }
            Self::SourceIdentityHashMismatch => {
                formatter.write_str("native source identity hash mismatch")
            }
            Self::TrustAnchorMismatch => formatter.write_str("native trust anchor mismatch"),
            Self::InvalidPayload => formatter.write_str("invalid canonical SCCP payload"),
            Self::PayloadLaneMismatch => {
                formatter.write_str("SCCP payload domains do not match the exact lane")
            }
            Self::MessageIdMismatch => formatter.write_str("SCCP message id mismatch"),
            Self::PayloadHashMismatch => formatter.write_str("SCCP payload hash mismatch"),
            Self::SourceEventDigestMismatch => {
                formatter.write_str("exact-lane SCCP source-event digest mismatch")
            }
            Self::HashRoleCollision => formatter.write_str("zero or colliding SCCP hash roles"),
            Self::Ethereum(error) => write!(formatter, "native Ethereum proof failed: {error}"),
            Self::Bsc(error) => write!(formatter, "native BSC proof failed: {error:?}"),
            Self::Solana(error) => write!(formatter, "native Solana proof failed: {error}"),
            Self::Tron(error) => write!(formatter, "native TRON proof failed: {error:?}"),
            Self::Ton(error) => write!(formatter, "native TON proof failed: {error}"),
            Self::NormalizedResultMismatch(role) => {
                write!(formatter, "native verifier returned mismatched {role}")
            }
        }
    }
}
impl std::error::Error for SccpNativeAdmissionErrorV1 {}
/// Return whether V1 safely admits an exact external profile as an inbound source.
///
/// This capability is intentionally directional. A `false` result does not
/// prevent a SORA-origin message from targeting the same external profile.
#[must_use]
pub const fn sccp_native_inbound_source_available_v1(network: SccpNetworkV1) -> bool {
    network.supports_native_inbound_source()
}
/// Map one admitted external source profile to its only native backend.
///
/// Networks outside the first-release Ethereum/BSC/TRON set return `None`.
#[must_use]
pub const fn sccp_native_backend_for_source_network_v1(
    network: SccpNetworkV1,
) -> Option<BridgeNativeProofBackendV1> {
    match network {
        SccpNetworkV1::EthereumMainnet | SccpNetworkV1::EthereumSepolia => {
            Some(BridgeNativeProofBackendV1::EthereumBeacon)
        }
        SccpNetworkV1::BscMainnet | SccpNetworkV1::BscTestnet => {
            Some(BridgeNativeProofBackendV1::BscParlia)
        }
        SccpNetworkV1::SolanaTestnet => Some(BridgeNativeProofBackendV1::SolanaAgave),
        SccpNetworkV1::TronMainnet | SccpNetworkV1::TronNile | SccpNetworkV1::TronShasta => {
            Some(BridgeNativeProofBackendV1::TronDpos)
        }
        SccpNetworkV1::TonMainnet | SccpNetworkV1::TonTestnet => {
            Some(BridgeNativeProofBackendV1::TonMasterchain)
        }
        SccpNetworkV1::SoraTaira => None,
    }
}
fn hashes_are_nonzero_and_distinct(hashes: &[H256]) -> bool {
    for (index, hash) in hashes.iter().enumerate() {
        if hash.iter().all(|byte| *byte == 0) || hashes[..index].contains(hash) {
            return false;
        }
    }
    true
}
fn validate_source_envelope_shape(
    envelope: &SccpNativeSourceProofEnvelopeV1,
) -> Result<H256, SccpNativeAdmissionErrorV1> {
    if envelope.version != 1 {
        return Err(SccpNativeAdmissionErrorV1::UnsupportedVersion(
            "native source envelope",
        ));
    }
    if !envelope.lane.is_well_formed()
        || !envelope.lane.source.is_external()
        || !envelope.lane.target.is_sora()
    {
        return Err(SccpNativeAdmissionErrorV1::InvalidLane);
    }
    if !sccp_native_inbound_source_available_v1(envelope.lane.source) {
        return Err(SccpNativeAdmissionErrorV1::SourceDirectionUnavailable {
            source: envelope.lane.source,
        });
    }
    let lane_hash =
        sccp_lane_id_hash_v1(envelope.lane).ok_or(SccpNativeAdmissionErrorV1::InvalidLane)?;
    let expected_backend = sccp_native_backend_for_source_network_v1(envelope.lane.source)
        .ok_or(SccpNativeAdmissionErrorV1::InvalidLane)?;
    if envelope.proof.backend() != expected_backend
        || envelope.trust_anchor.backend != expected_backend
        || envelope.proof.embedded_source_network() != envelope.lane.source
        || !envelope.trust_anchor.is_well_formed()
    {
        return Err(SccpNativeAdmissionErrorV1::BackendMismatch);
    }
    if envelope.source_finality.height == 0
        || !hashes_are_nonzero_and_distinct(&[
            lane_hash,
            envelope.source_identity_hash,
            envelope.trust_anchor.anchor_hash,
            envelope.message_id,
            envelope.payload_hash,
            envelope.source_event_digest,
            envelope.source_finality.block_hash,
        ])
    {
        return Err(SccpNativeAdmissionErrorV1::HashRoleCollision);
    }
    match &envelope.proof {
        SccpNativeSourceProofV1::EthereumBeacon(proof) => {
            if proof.source_identity.lane != envelope.lane
                || proof.source_identity_hash != envelope.source_identity_hash
                || proof.lane_hash != lane_hash
                || proof.trusted_anchor.network != envelope.lane.source
                || proof.trusted_anchor_hash != envelope.trust_anchor.anchor_hash
                || proof.message_id != envelope.message_id
                || proof.payload_hash != envelope.payload_hash
                || proof.source_event_digest != envelope.source_event_digest
            {
                return Err(SccpNativeAdmissionErrorV1::BackendMismatch);
            }
        }
        SccpNativeSourceProofV1::SolanaAgave(proof) => {
            if proof.statement.source_identity_hash != envelope.source_identity_hash
                || proof.statement.lane_hash != lane_hash
                || proof.statement.message_id != envelope.message_id
                || proof.statement.payload_hash != envelope.payload_hash
                || proof.statement.source_event_digest != envelope.source_event_digest
                || proof.statement.rooted_slot != envelope.source_finality.height
                || proof.statement.rooted_bank_hash != envelope.source_finality.block_hash
            {
                return Err(SccpNativeAdmissionErrorV1::BackendMismatch);
            }
        }
        SccpNativeSourceProofV1::BscParlia(_)
        | SccpNativeSourceProofV1::TronDpos(_)
        | SccpNativeSourceProofV1::TonMasterchain(_) => {}
    }
    Ok(lane_hash)
}
fn validate_inbound_statement(
    proof: &SccpNativeInboundMessageProofV1,
) -> Result<H256, SccpNativeAdmissionErrorV1> {
    if proof.version != 1 {
        return Err(SccpNativeAdmissionErrorV1::UnsupportedVersion(
            "native inbound proof",
        ));
    }
    let lane_hash = validate_source_envelope_shape(&proof.source)?;
    if !verify_sccp_payload_structure(&proof.payload) {
        return Err(SccpNativeAdmissionErrorV1::InvalidPayload);
    }
    let canonical_payload = canonical_sccp_payload_bytes(&proof.payload)
        .map_err(|_| SccpNativeAdmissionErrorV1::InvalidPayload)?;
    if canonical_payload.is_empty()
        || canonical_payload.len() > SCCP_NATIVE_ADMISSION_MAX_PAYLOAD_BYTES_V1
    {
        return Err(SccpNativeAdmissionErrorV1::InvalidPayload);
    }
    if sccp_message_source_domain(&proof.payload) != proof.source.lane.source.domain_id()
        || sccp_message_target_domain(&proof.payload) != proof.source.lane.target.domain_id()
    {
        return Err(SccpNativeAdmissionErrorV1::PayloadLaneMismatch);
    }
    let message_id = sccp_message_id(proof.source.lane, &proof.payload)
        .ok_or(SccpNativeAdmissionErrorV1::PayloadLaneMismatch)?;
    if proof.source.message_id != message_id {
        return Err(SccpNativeAdmissionErrorV1::MessageIdMismatch);
    }
    let canonical_payload_hash = payload_hash(&canonical_payload);
    if proof.source.payload_hash != canonical_payload_hash {
        return Err(SccpNativeAdmissionErrorV1::PayloadHashMismatch);
    }
    let event_digest =
        sccp_lane_source_event_digest_v1(proof.source.lane, message_id, canonical_payload_hash)
            .ok_or(SccpNativeAdmissionErrorV1::SourceEventDigestMismatch)?;
    if proof.source.source_event_digest != event_digest {
        return Err(SccpNativeAdmissionErrorV1::SourceEventDigestMismatch);
    }
    Ok(lane_hash)
}
fn check_encoded_size(bytes: &[u8]) -> Result<(), SccpNativeAdmissionErrorV1> {
    if bytes.is_empty() {
        return Err(SccpNativeAdmissionErrorV1::EmptyEncoding);
    }
    if bytes.len() > SCCP_NATIVE_ADMISSION_MAX_ENCODED_BYTES_V1 {
        return Err(SccpNativeAdmissionErrorV1::EncodedSize {
            actual: bytes.len(),
            maximum: SCCP_NATIVE_ADMISSION_MAX_ENCODED_BYTES_V1,
        });
    }
    Ok(())
}
fn preflight_canonical_norito(bytes: &[u8]) -> Result<(), SccpNativeAdmissionErrorV1> {
    check_encoded_size(bytes)?;
    // V1 admits one uncompressed encoding. Rejecting compression and a large
    // declared uncompressed length before generic decoding prevents a compact
    // decompression bomb from bypassing the outer input-size bound.
    if bytes.len() < norito::core::Header::SIZE
        || bytes.get(..4) != Some(b"NRT0")
        || bytes.get(NORITO_COMPRESSION_OFFSET) != Some(&0)
    {
        return Err(SccpNativeAdmissionErrorV1::InvalidNoritoEncoding);
    }
    let declared_length = bytes
        .get(NORITO_LENGTH_OFFSET..NORITO_LENGTH_OFFSET + 8)
        .and_then(|raw| <[u8; 8]>::try_from(raw).ok())
        .map(u64::from_le_bytes)
        .ok_or(SccpNativeAdmissionErrorV1::InvalidNoritoEncoding)?;
    if declared_length > SCCP_NATIVE_ADMISSION_MAX_ENCODED_BYTES_U64_V1 {
        return Err(SccpNativeAdmissionErrorV1::EncodedSize {
            actual: usize::try_from(declared_length).unwrap_or(usize::MAX),
            maximum: SCCP_NATIVE_ADMISSION_MAX_ENCODED_BYTES_V1,
        });
    }
    Ok(())
}
/// Encode one structurally valid source envelope as canonical Norito bytes.
///
/// # Errors
///
/// Returns an error if the envelope is structurally invalid, cannot be encoded
/// canonically, or exceeds the V1 encoded-size limit.
pub fn encode_sccp_native_source_proof_envelope_v1(
    envelope: &SccpNativeSourceProofEnvelopeV1,
) -> Result<Vec<u8>, SccpNativeAdmissionErrorV1> {
    validate_source_envelope_shape(envelope)?;
    let encoded = norito::to_bytes(envelope)
        .map_err(|_| SccpNativeAdmissionErrorV1::InvalidNoritoEncoding)?;
    check_encoded_size(&encoded)?;
    Ok(encoded)
}
/// Decode exactly one canonical, size-bounded native source envelope.
///
/// # Errors
///
/// Returns an error for empty, oversized, compressed, malformed,
/// noncanonical, or structurally invalid encodings.
pub fn decode_sccp_native_source_proof_envelope_v1(
    bytes: &[u8],
) -> Result<SccpNativeSourceProofEnvelopeV1, SccpNativeAdmissionErrorV1> {
    preflight_canonical_norito(bytes)?;
    let envelope: SccpNativeSourceProofEnvelopeV1 = norito::decode_from_bytes(bytes)
        .map_err(|_| SccpNativeAdmissionErrorV1::InvalidNoritoEncoding)?;
    let canonical = encode_sccp_native_source_proof_envelope_v1(&envelope)?;
    if canonical != bytes {
        return Err(SccpNativeAdmissionErrorV1::InvalidNoritoEncoding);
    }
    Ok(envelope)
}
/// Decode one strict, size-bounded JSON native source envelope.
///
/// # Errors
///
/// Returns an error for empty, oversized, malformed, noncanonical, or structurally invalid JSON.
pub fn decode_sccp_native_source_proof_envelope_json_v1(
    json: &str,
) -> Result<SccpNativeSourceProofEnvelopeV1, SccpNativeAdmissionErrorV1> {
    if json.is_empty() {
        return Err(SccpNativeAdmissionErrorV1::EmptyEncoding);
    }
    if json.len() > SCCP_NATIVE_ADMISSION_MAX_JSON_BYTES_V1 {
        return Err(SccpNativeAdmissionErrorV1::EncodedSize {
            actual: json.len(),
            maximum: SCCP_NATIVE_ADMISSION_MAX_JSON_BYTES_V1,
        });
    }
    let envelope = norito::json::from_str::<SccpNativeSourceProofEnvelopeV1>(json)
        .map_err(|_| SccpNativeAdmissionErrorV1::InvalidJsonEncoding)?;
    let canonical = norito::json::to_json(&envelope)
        .map_err(|_| SccpNativeAdmissionErrorV1::InvalidJsonEncoding)?;
    if canonical != json {
        return Err(SccpNativeAdmissionErrorV1::InvalidJsonEncoding);
    }
    encode_sccp_native_source_proof_envelope_v1(&envelope)?;
    Ok(envelope)
}
/// Encode one structurally valid native inbound proof as canonical Norito bytes.
///
/// # Errors
///
/// Returns an error if the statement is invalid, cannot be encoded
/// canonically, or exceeds the V1 encoded-size limit.
pub fn encode_sccp_native_inbound_message_proof_v1(
    proof: &SccpNativeInboundMessageProofV1,
) -> Result<Vec<u8>, SccpNativeAdmissionErrorV1> {
    validate_inbound_statement(proof)?;
    let encoded =
        norito::to_bytes(proof).map_err(|_| SccpNativeAdmissionErrorV1::InvalidNoritoEncoding)?;
    check_encoded_size(&encoded)?;
    Ok(encoded)
}
/// Decode exactly one canonical, size-bounded native inbound proof.
///
/// # Errors
///
/// Returns an error for empty, oversized, compressed, malformed,
/// noncanonical, or structurally invalid encodings.
pub fn decode_sccp_native_inbound_message_proof_v1(
    bytes: &[u8],
) -> Result<SccpNativeInboundMessageProofV1, SccpNativeAdmissionErrorV1> {
    preflight_canonical_norito(bytes)?;
    let proof: SccpNativeInboundMessageProofV1 = norito::decode_from_bytes(bytes)
        .map_err(|_| SccpNativeAdmissionErrorV1::InvalidNoritoEncoding)?;
    let canonical = encode_sccp_native_inbound_message_proof_v1(&proof)?;
    if canonical != bytes {
        return Err(SccpNativeAdmissionErrorV1::InvalidNoritoEncoding);
    }
    Ok(proof)
}
/// Decode one strict, size-bounded JSON native inbound proof.
///
/// # Errors
///
/// Returns an error for empty, oversized, malformed, noncanonical, or structurally invalid JSON.
pub fn decode_sccp_native_inbound_message_proof_json_v1(
    json: &str,
) -> Result<SccpNativeInboundMessageProofV1, SccpNativeAdmissionErrorV1> {
    if json.is_empty() {
        return Err(SccpNativeAdmissionErrorV1::EmptyEncoding);
    }
    if json.len() > SCCP_NATIVE_ADMISSION_MAX_JSON_BYTES_V1 {
        return Err(SccpNativeAdmissionErrorV1::EncodedSize {
            actual: json.len(),
            maximum: SCCP_NATIVE_ADMISSION_MAX_JSON_BYTES_V1,
        });
    }
    let proof = norito::json::from_str::<SccpNativeInboundMessageProofV1>(json)
        .map_err(|_| SccpNativeAdmissionErrorV1::InvalidJsonEncoding)?;
    let canonical = norito::json::to_json(&proof)
        .map_err(|_| SccpNativeAdmissionErrorV1::InvalidJsonEncoding)?;
    if canonical != json {
        return Err(SccpNativeAdmissionErrorV1::InvalidJsonEncoding);
    }
    encode_sccp_native_inbound_message_proof_v1(&proof)?;
    Ok(proof)
}
/// Build the data-model container for one canonical native inbound proof.
///
/// # Errors
///
/// Returns an error if the route-configuration hash is zero or the embedded
/// inbound proof is invalid or cannot be canonically encoded.
pub fn bridge_native_protocol_proof_v1(
    proof: &SccpNativeInboundMessageProofV1,
    route_configuration_hash: [u8; 32],
) -> Result<BridgeNativeProtocolProofV1, SccpNativeAdmissionErrorV1> {
    if route_configuration_hash.iter().all(|byte| *byte == 0) {
        return Err(SccpNativeAdmissionErrorV1::HashRoleCollision);
    }
    let encoded_envelope = encode_sccp_native_inbound_message_proof_v1(proof)?;
    Ok(BridgeNativeProtocolProofV1 {
        backend: proof.source.proof.backend(),
        route_configuration_hash,
        encoded_envelope,
    })
}
/// Decode a data-model native container and require its outer backend to match.
///
/// # Errors
///
/// Returns an error if the route-configuration hash is zero, the embedded
/// proof is invalid, or the outer and inner backend tags disagree.
pub fn decode_bridge_native_protocol_proof_v1(
    proof: &BridgeNativeProtocolProofV1,
) -> Result<SccpNativeInboundMessageProofV1, SccpNativeAdmissionErrorV1> {
    if proof.route_configuration_hash.iter().all(|byte| *byte == 0) {
        return Err(SccpNativeAdmissionErrorV1::HashRoleCollision);
    }
    let decoded = decode_sccp_native_inbound_message_proof_v1(&proof.encoded_envelope)?;
    if decoded.source.proof.backend() != proof.backend {
        return Err(SccpNativeAdmissionErrorV1::BackendMismatch);
    }
    Ok(decoded)
}
fn validate_governed_context(
    proof: &SccpNativeInboundMessageProofV1,
    governed_source_identity: &SccpSourceIdentityV1,
    governed_trust_anchor: SccpNativeTrustAnchorV1,
) -> Result<(), SccpNativeAdmissionErrorV1> {
    if !governed_source_identity.is_well_formed()
        || governed_source_identity.lane != proof.source.lane
    {
        return Err(SccpNativeAdmissionErrorV1::InvalidSourceIdentity);
    }
    let identity_hash = sccp_source_identity_hash_v1(governed_source_identity)
        .ok_or(SccpNativeAdmissionErrorV1::InvalidSourceIdentity)?;
    if identity_hash != proof.source.source_identity_hash {
        return Err(SccpNativeAdmissionErrorV1::SourceIdentityHashMismatch);
    }
    if !governed_trust_anchor.is_well_formed() || governed_trust_anchor != proof.source.trust_anchor
    {
        return Err(SccpNativeAdmissionErrorV1::TrustAnchorMismatch);
    }
    Ok(())
}
fn normalized_result(
    proof: &SccpNativeInboundMessageProofV1,
    lane_hash: H256,
    source_finality: SccpNativeFinalityPointV1,
    anchor_interval_height: u64,
) -> Result<ValidatedSccpNativeInboundMessageV1, SccpNativeAdmissionErrorV1> {
    if source_finality.height == 0
        || anchor_interval_height == 0
        || !hashes_are_nonzero_and_distinct(&[
            lane_hash,
            proof.source.source_identity_hash,
            proof.source.trust_anchor.anchor_hash,
            proof.source.message_id,
            proof.source.payload_hash,
            proof.source.source_event_digest,
            source_finality.block_hash,
        ])
    {
        return Err(SccpNativeAdmissionErrorV1::HashRoleCollision);
    }
    let message_key = SccpInboundMessageKeyV1::new(proof.source.lane, proof.source.message_id)
        .ok_or(SccpNativeAdmissionErrorV1::InvalidLane)?;
    Ok(ValidatedSccpNativeInboundMessageV1 {
        message_key,
        lane_hash,
        source_identity_hash: proof.source.source_identity_hash,
        trust_anchor: proof.source.trust_anchor,
        payload_hash: proof.source.payload_hash,
        source_event_digest: proof.source.source_event_digest,
        source_finality,
        anchor_interval_height,
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VerifiedNativeFinalityV1 {
    source_finality: SccpNativeFinalityPointV1,
    anchor_interval_height: u64,
}
struct GovernedNativeAdmissionContextV1<'a> {
    proof: &'a SccpNativeInboundMessageProofV1,
    source_identity: &'a SccpSourceIdentityV1,
    trust_anchor: SccpNativeTrustAnchorV1,
    lane_hash: H256,
}
fn verify_ethereum_native_admission_v1(
    context: &GovernedNativeAdmissionContextV1<'_>,
    native: &EthereumNativeSourceProofV1,
) -> Result<VerifiedNativeFinalityV1, SccpNativeAdmissionErrorV1> {
    if native.trusted_anchor.bootstrap.header.beacon.slot != context.trust_anchor.checkpoint_height
    {
        return Err(SccpNativeAdmissionErrorV1::TrustAnchorMismatch);
    }
    let proof = context.proof;
    let canonical_payload = canonical_sccp_payload_bytes(&proof.payload)
        .map_err(|_| SccpNativeAdmissionErrorV1::InvalidPayload)?;
    let validated = verify_ethereum_native_source_proof_v1(
        context.source_identity,
        proof.source.source_identity_hash,
        proof.source.trust_anchor.anchor_hash,
        proof.source.message_id,
        proof.source.payload_hash,
        &canonical_payload,
        native,
    )
    .map_err(SccpNativeAdmissionErrorV1::Ethereum)?;
    if validated.source_identity_hash != proof.source.source_identity_hash
        || validated.lane_hash != context.lane_hash
        || validated.trusted_anchor_hash != proof.source.trust_anchor.anchor_hash
        || validated.message_id != proof.source.message_id
        || validated.payload_hash != proof.source.payload_hash
        || validated.source_event_digest != proof.source.source_event_digest
    {
        return Err(SccpNativeAdmissionErrorV1::NormalizedResultMismatch(
            "Ethereum statement",
        ));
    }
    Ok(VerifiedNativeFinalityV1 {
        source_finality: SccpNativeFinalityPointV1 {
            height: validated.execution_block_number,
            block_hash: validated.execution_block_hash,
        },
        anchor_interval_height: validated.finalized_beacon_slot,
    })
}
fn verify_bsc_native_admission_v1(
    context: &GovernedNativeAdmissionContextV1<'_>,
    native: &BscNativeSourceProofV1,
) -> Result<VerifiedNativeFinalityV1, SccpNativeAdmissionErrorV1> {
    if bsc_native_anchor_block_number(&native.finality.anchor)
        .map_err(BscNativeSourceError::Finality)
        .map_err(SccpNativeAdmissionErrorV1::Bsc)?
        != context.trust_anchor.checkpoint_height
    {
        return Err(SccpNativeAdmissionErrorV1::TrustAnchorMismatch);
    }
    let proof = context.proof;
    let canonical_payload = canonical_sccp_payload_bytes(&proof.payload)
        .map_err(|_| SccpNativeAdmissionErrorV1::InvalidPayload)?;
    let validated = verify_bsc_native_source(
        native,
        context.source_identity,
        proof.source.source_identity_hash,
        proof.source.trust_anchor.anchor_hash,
        proof.source.message_id,
        proof.source.payload_hash,
        &canonical_payload,
    )
    .map_err(SccpNativeAdmissionErrorV1::Bsc)?;
    if validated.source_identity_hash != proof.source.source_identity_hash
        || validated.lane_hash != context.lane_hash
        || validated.finality.anchor_hash != proof.source.trust_anchor.anchor_hash
        || validated.receipt.lane_hash != context.lane_hash
        || validated.receipt.source_event_digest != proof.source.source_event_digest
    {
        return Err(SccpNativeAdmissionErrorV1::NormalizedResultMismatch(
            "BSC statement",
        ));
    }
    Ok(VerifiedNativeFinalityV1 {
        source_finality: SccpNativeFinalityPointV1 {
            height: validated.finality.block_number,
            block_hash: validated.finality.block_hash,
        },
        anchor_interval_height: validated.finality.block_number,
    })
}
fn verify_tron_native_admission_v1(
    context: &GovernedNativeAdmissionContextV1<'_>,
    native: &TronNativeSourceProofV1,
) -> Result<VerifiedNativeFinalityV1, SccpNativeAdmissionErrorV1> {
    if native.finality.anchor.block_number != context.trust_anchor.checkpoint_height {
        return Err(SccpNativeAdmissionErrorV1::TrustAnchorMismatch);
    }
    let proof = context.proof;
    let validated = verify_tron_native_source(
        native,
        context.source_identity,
        proof.source.source_identity_hash,
        proof.source.trust_anchor.anchor_hash,
        proof.source.message_id,
        proof.source.payload_hash,
        &proof.payload,
    )
    .map_err(SccpNativeAdmissionErrorV1::Tron)?;
    if validated.source_identity_hash != proof.source.source_identity_hash
        || validated.finality.anchor_hash != proof.source.trust_anchor.anchor_hash
        || validated.transaction.lane_hash != context.lane_hash
        || validated.transaction.message_id != proof.source.message_id
        || validated.transaction.payload_hash != proof.source.payload_hash
        || validated.transaction.source_event_digest != proof.source.source_event_digest
    {
        return Err(SccpNativeAdmissionErrorV1::NormalizedResultMismatch(
            "TRON statement",
        ));
    }
    Ok(VerifiedNativeFinalityV1 {
        source_finality: SccpNativeFinalityPointV1 {
            height: validated.finality.block_number,
            block_hash: validated.finality.block_id,
        },
        anchor_interval_height: validated.finality.block_number,
    })
}
fn verify_ton_native_admission_v1(
    context: &GovernedNativeAdmissionContextV1<'_>,
    native: &TonNativeSourceProofV1,
) -> Result<VerifiedNativeFinalityV1, SccpNativeAdmissionErrorV1> {
    if u64::from(native.finality.anchor.checkpoint.seqno) != context.trust_anchor.checkpoint_height
    {
        return Err(SccpNativeAdmissionErrorV1::TrustAnchorMismatch);
    }
    let proof = context.proof;
    let validated = verify_ton_native_source(
        native,
        context.source_identity,
        proof.source.source_identity_hash,
        proof.source.trust_anchor.anchor_hash,
        proof.source.message_id,
        proof.source.payload_hash,
        &proof.payload,
    )
    .map_err(SccpNativeAdmissionErrorV1::Ton)?;
    if validated.source_identity_hash != proof.source.source_identity_hash
        || validated.lane_hash != context.lane_hash
        || validated.anchor_hash != proof.source.trust_anchor.anchor_hash
        || validated.message_id != proof.source.message_id
        || validated.payload_hash != proof.source.payload_hash
        || validated.source_event_digest != proof.source.source_event_digest
    {
        return Err(SccpNativeAdmissionErrorV1::NormalizedResultMismatch(
            "TON statement",
        ));
    }
    Ok(VerifiedNativeFinalityV1 {
        source_finality: SccpNativeFinalityPointV1 {
            height: u64::from(validated.shard_seqno),
            block_hash: validated.shard_block_hash,
        },
        anchor_interval_height: u64::from(validated.masterchain_seqno),
    })
}
fn verify_solana_native_admission_v1(
    context: &GovernedNativeAdmissionContextV1<'_>,
    native: &SccpSolanaAgaveSourceProofV1,
) -> Result<VerifiedNativeFinalityV1, SccpNativeAdmissionErrorV1> {
    if native.anchor.checkpoint_slot != context.trust_anchor.checkpoint_height {
        return Err(SccpNativeAdmissionErrorV1::TrustAnchorMismatch);
    }
    let proof = context.proof;
    let validated = verify_sccp_solana_agave_source_v1(
        native,
        context.source_identity,
        proof.source.source_identity_hash,
        proof.source.trust_anchor.anchor_hash,
        proof.source.message_id,
        proof.source.payload_hash,
        &proof.payload,
    )
    .map_err(SccpNativeAdmissionErrorV1::Solana)?;
    if validated.source_identity_hash != proof.source.source_identity_hash
        || validated.lane_hash != context.lane_hash
        || validated.anchor_hash != proof.source.trust_anchor.anchor_hash
        || validated.source_event_digest != proof.source.source_event_digest
    {
        return Err(SccpNativeAdmissionErrorV1::NormalizedResultMismatch(
            "Solana statement",
        ));
    }
    Ok(VerifiedNativeFinalityV1 {
        source_finality: SccpNativeFinalityPointV1 {
            height: validated.rooted_slot,
            block_hash: validated.rooted_bank_hash,
        },
        anchor_interval_height: validated.rooted_slot,
    })
}
/// Verify a complete native inbound proof against governed lane material.
///
/// Every branch invokes its full protocol-native verifier. Success is returned
/// only after the verifier's normalized identity, anchor, lane, event, and
/// source-finality fields are compared with the exact admitted statement.
///
/// # Errors
///
/// Returns an error if the statement or governed context is inconsistent, a protocol-native
/// verifier fails, or its normalized result does not exactly match the admitted proof envelope.
pub fn verify_sccp_native_inbound_message_proof_v1(
    proof: &SccpNativeInboundMessageProofV1,
    governed_source_identity: &SccpSourceIdentityV1,
    governed_trust_anchor: SccpNativeTrustAnchorV1,
) -> Result<ValidatedSccpNativeInboundMessageV1, SccpNativeAdmissionErrorV1> {
    let lane_hash = validate_inbound_statement(proof)?;
    validate_governed_context(proof, governed_source_identity, governed_trust_anchor)?;
    let context = GovernedNativeAdmissionContextV1 {
        proof,
        source_identity: governed_source_identity,
        trust_anchor: governed_trust_anchor,
        lane_hash,
    };
    let verified_finality = match &proof.source.proof {
        SccpNativeSourceProofV1::EthereumBeacon(native) => {
            verify_ethereum_native_admission_v1(&context, native)?
        }
        SccpNativeSourceProofV1::BscParlia(native) => {
            verify_bsc_native_admission_v1(&context, native)?
        }
        SccpNativeSourceProofV1::SolanaAgave(native) => {
            verify_solana_native_admission_v1(&context, native)?
        }
        SccpNativeSourceProofV1::TronDpos(native) => {
            verify_tron_native_admission_v1(&context, native)?
        }
        SccpNativeSourceProofV1::TonMasterchain(native) => {
            verify_ton_native_admission_v1(&context, native)?
        }
    };
    if verified_finality.source_finality != proof.source.source_finality {
        return Err(SccpNativeAdmissionErrorV1::NormalizedResultMismatch(
            "source finality",
        ));
    }
    normalized_result(
        proof,
        lane_hash,
        verified_finality.source_finality,
        verified_finality.anchor_interval_height,
    )
}
/// Build a complete positive Ethereum native inbound fixture for one payload.
///
/// The payload must be a structurally valid Ethereum-to-SORA SCCP message. The builder derives its
/// message id and payload hash first, then rebuilds the authenticated Ethereum event receipt,
/// receipt trie, execution header, beacon header, trusted anchor, and every outer commitment for
/// that exact statement. It never mutates a pre-existing proof after authentication.
///
/// # Errors
///
/// Returns a typed admission error when the payload is invalid or bound to a
/// different lane, the constructed native proof fails source verification, or
/// any normalized fixture commitment disagrees with the requested statement.
#[cfg(any(test, feature = "test-fixtures"))]
pub fn sccp_native_ethereum_inbound_test_fixture_for_payload_v1(
    payload: SccpPayloadV1,
) -> Result<
    (
        SccpNativeInboundMessageProofV1,
        SccpSourceIdentityV1,
        SccpNativeTrustAnchorV1,
    ),
    SccpNativeAdmissionErrorV1,
> {
    if !verify_sccp_payload_structure(&payload) {
        return Err(SccpNativeAdmissionErrorV1::InvalidPayload);
    }
    let canonical_payload = canonical_sccp_payload_bytes(&payload)
        .map_err(|_| SccpNativeAdmissionErrorV1::InvalidPayload)?;
    if canonical_payload.is_empty()
        || canonical_payload.len() > SCCP_NATIVE_ADMISSION_MAX_PAYLOAD_BYTES_V1
    {
        return Err(SccpNativeAdmissionErrorV1::InvalidPayload);
    }
    let fixture_lane = SccpLaneIdV1 {
        source: SccpNetworkV1::EthereumMainnet,
        target: SccpNetworkV1::SoraTaira,
    };
    let message_id = sccp_message_id(fixture_lane, &payload)
        .ok_or(SccpNativeAdmissionErrorV1::PayloadLaneMismatch)?;
    let canonical_payload_hash = payload_hash(&canonical_payload);
    let (identity, identity_hash, anchor_hash, fixture_message_id, fixture_payload_hash, native) =
        crate::ethereum_source::ethereum_native_positive_test_fixture_for_statement(
            message_id,
            &canonical_payload,
        );
    if fixture_message_id != message_id || fixture_payload_hash != canonical_payload_hash {
        return Err(SccpNativeAdmissionErrorV1::NormalizedResultMismatch(
            "Ethereum test fixture statement",
        ));
    }
    if identity.lane != fixture_lane {
        return Err(SccpNativeAdmissionErrorV1::PayloadLaneMismatch);
    }
    if sccp_message_source_domain(&payload) != identity.lane.source.domain_id()
        || sccp_message_target_domain(&payload) != identity.lane.target.domain_id()
    {
        return Err(SccpNativeAdmissionErrorV1::PayloadLaneMismatch);
    }
    let source_event_digest =
        sccp_lane_source_event_digest_v1(identity.lane, fixture_message_id, fixture_payload_hash)
            .ok_or(SccpNativeAdmissionErrorV1::SourceEventDigestMismatch)?;
    let checkpoint_height = native.trusted_anchor.bootstrap.header.beacon.slot;
    let trust_anchor = SccpNativeTrustAnchorV1 {
        backend: BridgeNativeProofBackendV1::EthereumBeacon,
        checkpoint_height,
        anchor_hash,
    };
    let validated_native = verify_ethereum_native_source_proof_v1(
        &identity,
        identity_hash,
        anchor_hash,
        fixture_message_id,
        fixture_payload_hash,
        &canonical_payload,
        &native,
    )
    .map_err(SccpNativeAdmissionErrorV1::Ethereum)?;
    let source_finality = SccpNativeFinalityPointV1 {
        height: validated_native.execution_block_number,
        block_hash: validated_native.execution_block_hash,
    };
    let proof = SccpNativeInboundMessageProofV1 {
        version: 1,
        payload,
        source: SccpNativeSourceProofEnvelopeV1 {
            version: 1,
            lane: identity.lane,
            source_identity_hash: identity_hash,
            trust_anchor,
            message_id: fixture_message_id,
            payload_hash: fixture_payload_hash,
            source_event_digest,
            source_finality,
            proof: SccpNativeSourceProofV1::EthereumBeacon(Box::new(native)),
        },
    };
    validate_inbound_statement(&proof)?;
    Ok((proof, identity, trust_anchor))
}
/// Build a complete positive Ethereum native transfer fixture.
///
/// This helper exists only for crate and downstream integration tests compiled
/// with the `test-fixtures` feature.
#[cfg(any(test, feature = "test-fixtures"))]
#[must_use]
pub fn sccp_native_ethereum_inbound_test_fixture_v1() -> (
    SccpNativeInboundMessageProofV1,
    SccpSourceIdentityV1,
    SccpNativeTrustAnchorV1,
) {
    let payload = ethereum_transfer_test_payload();
    sccp_native_ethereum_inbound_test_fixture_for_payload_v1(payload)
        .expect("fixed Ethereum transfer test fixture is valid")
}
/// Build a complete positive Ethereum native transfer fixture.
///
/// The returned message is an Ethereum mainnet to SORA Taira transfer and is
/// suitable for receipt, replay, settlement, and Torii integration tests.
#[cfg(any(test, feature = "test-fixtures"))]
#[must_use]
pub fn sccp_native_ethereum_transfer_inbound_test_fixture_v1() -> (
    SccpNativeInboundMessageProofV1,
    SccpSourceIdentityV1,
    SccpNativeTrustAnchorV1,
) {
    sccp_native_ethereum_inbound_test_fixture_v1()
}
#[cfg(any(test, feature = "test-fixtures"))]
fn ethereum_transfer_test_payload() -> SccpPayloadV1 {
    let recipient = iroha_data_model::account::AccountId::new(
        iroha_crypto::KeyPair::try_from_seed(vec![0x91; 32], iroha_crypto::Algorithm::Ed25519)
            .expect("exact native SCCP recipient fixture key")
            .public_key()
            .clone(),
    )
    .to_account_address()
    .and_then(|address| address.to_i105_for_discriminant(crate::SCCP_TAIRA_I105_DISCRIMINANT_V1))
    .expect("exact native SCCP recipient fixture has canonical Taira I105");
    SccpPayloadV1::Transfer(crate::TransferPayloadV1 {
        version: 1,
        source_domain: crate::SCCP_DOMAIN_ETH,
        dest_domain: crate::SCCP_DOMAIN_SORA,
        nonce: 11,
        route_revision: 1,
        asset_home_domain: crate::SCCP_DOMAIN_SORA,
        asset_id_codec: crate::SCCP_CODEC_CANONICAL_TEXT,
        asset_id: crate::SCCP_TAIRA_XOR_ASSET_KEY_V1.as_bytes().to_vec(),
        amount: 1_000_000_000,
        sender_codec: crate::SCCP_CODEC_EVM_ADDRESS20,
        sender: vec![0x52; 20],
        recipient_codec: crate::SCCP_CODEC_CANONICAL_TEXT,
        recipient: recipient.into_bytes(),
        route_id_codec: crate::SCCP_CODEC_CANONICAL_TEXT,
        route_id: crate::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1.as_bytes().to_vec(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EthereumNativeMptRoleV1, SCCP_DOMAIN_ETH, SCCP_DOMAIN_SORA};
    use norito::codec::{DecodeAll as _, Encode as _};
    fn inbound_fixture() -> (
        SccpNativeInboundMessageProofV1,
        SccpSourceIdentityV1,
        SccpNativeTrustAnchorV1,
    ) {
        sccp_native_ethereum_inbound_test_fixture_v1()
    }
    fn ethereum_proof_mut(
        proof: &mut SccpNativeInboundMessageProofV1,
    ) -> &mut EthereumNativeSourceProofV1 {
        let SccpNativeSourceProofV1::EthereumBeacon(native) = &mut proof.source.proof else {
            panic!("fixture uses Ethereum")
        };
        native.as_mut()
    }
    #[test]
    fn full_ethereum_dispatch_normalizes_exact_lane_statement() {
        let (proof, identity, trust_anchor) = inbound_fixture();
        let validated =
            verify_sccp_native_inbound_message_proof_v1(&proof, &identity, trust_anchor)
                .expect("complete native Ethereum proof verifies");
        assert_eq!(validated.message_key.lane, identity.lane);
        assert_eq!(validated.message_key.message_id, proof.source.message_id);
        assert_eq!(
            validated.lane_hash,
            sccp_lane_id_hash_v1(identity.lane).unwrap()
        );
        assert_eq!(
            validated.source_identity_hash,
            proof.source.source_identity_hash
        );
        assert_eq!(validated.trust_anchor, trust_anchor);
        assert_eq!(validated.payload_hash, proof.source.payload_hash);
        assert_eq!(
            validated.source_event_digest,
            proof.source.source_event_digest
        );
        assert_eq!(validated.source_finality.height, 17_000_000);
        assert_eq!(validated.source_finality.block_hash, [7; 32]);
        assert!(validated.anchor_interval_height >= trust_anchor.checkpoint_height);
        assert_ne!(
            validated.anchor_interval_height, validated.source_finality.height,
            "Ethereum anchor intervals use finalized beacon slots, not execution block numbers"
        );
    }
    #[test]
    fn exported_ethereum_transfer_fixture_rebuilds_and_verifies() {
        let (proof, identity, trust_anchor) =
            sccp_native_ethereum_transfer_inbound_test_fixture_v1();
        let SccpPayloadV1::Transfer(payload) = &proof.payload;
        assert_eq!(payload.source_domain, SCCP_DOMAIN_ETH);
        assert_eq!(payload.dest_domain, SCCP_DOMAIN_SORA);
        assert_ne!(payload.amount, 0);
        let validated =
            verify_sccp_native_inbound_message_proof_v1(&proof, &identity, trust_anchor)
                .expect("exported transfer fixture verifies through full native dispatch");
        assert_eq!(validated.message_key.lane, identity.lane);
        assert_eq!(validated.message_key.message_id, proof.source.message_id);
    }
    #[test]
    fn canonical_binary_json_and_bridge_container_roundtrip() {
        let (proof, _, _) = inbound_fixture();
        let encoded = encode_sccp_native_inbound_message_proof_v1(&proof)
            .expect("canonical inbound proof encodes");
        assert_eq!(
            decode_sccp_native_inbound_message_proof_v1(&encoded),
            Ok(proof.clone())
        );
        let envelope = encode_sccp_native_source_proof_envelope_v1(&proof.source)
            .expect("canonical source envelope encodes");
        assert_eq!(
            decode_sccp_native_source_proof_envelope_v1(&envelope),
            Ok(proof.source.clone())
        );
        let json = norito::json::to_json(&proof).expect("native proof JSON encodes");
        assert_eq!(
            decode_sccp_native_inbound_message_proof_json_v1(&json),
            Ok(proof.clone())
        );
        let route_configuration_hash = [0x91; 32];
        let container = bridge_native_protocol_proof_v1(&proof, route_configuration_hash)
            .expect("native data-model container is canonical");
        assert_eq!(
            container.backend,
            BridgeNativeProofBackendV1::EthereumBeacon
        );
        assert_eq!(container.route_configuration_hash, route_configuration_hash);
        assert_eq!(
            decode_bridge_native_protocol_proof_v1(&container),
            Ok(proof.clone())
        );
        let encoded_container = container.encode();
        let decoded_container =
            BridgeNativeProtocolProofV1::decode_all(&mut encoded_container.as_slice())
                .expect("data-model native container decodes");
        assert_eq!(decoded_container, container);
        let anchor_bytes = proof.source.trust_anchor.encode();
        let decoded_anchor = SccpNativeTrustAnchorV1::decode_all(&mut anchor_bytes.as_slice())
            .expect("data-model native trust anchor decodes");
        assert_eq!(decoded_anchor, proof.source.trust_anchor);
        assert!(decoded_anchor.is_well_formed());
        assert!(
            !SccpNativeTrustAnchorV1 {
                anchor_hash: [0; 32],
                ..decoded_anchor
            }
            .is_well_formed()
        );
        assert!(
            !SccpNativeTrustAnchorV1 {
                checkpoint_height: 0,
                ..decoded_anchor
            }
            .is_well_formed()
        );
        let mut unknown_backend = BridgeNativeProofBackendV1::EthereumBeacon.encode();
        let unknown_tag = u32::MAX.encode();
        unknown_backend[..unknown_tag.len()].copy_from_slice(&unknown_tag);
        assert!(BridgeNativeProofBackendV1::decode_all(&mut unknown_backend.as_slice()).is_err());
        let mut wrong_backend = container;
        wrong_backend.backend = BridgeNativeProofBackendV1::BscParlia;
        assert_eq!(
            decode_bridge_native_protocol_proof_v1(&wrong_backend),
            Err(SccpNativeAdmissionErrorV1::BackendMismatch)
        );
        assert_eq!(
            bridge_native_protocol_proof_v1(&proof, [0; 32]),
            Err(SccpNativeAdmissionErrorV1::HashRoleCollision)
        );
        let mut zero_route = wrong_backend;
        zero_route.backend = BridgeNativeProofBackendV1::EthereumBeacon;
        zero_route.route_configuration_hash = [0; 32];
        assert_eq!(
            decode_bridge_native_protocol_proof_v1(&zero_route),
            Err(SccpNativeAdmissionErrorV1::HashRoleCollision)
        );
    }
    #[test]
    fn canonical_binary_decoder_rejects_truncation_trailing_compression_and_bombs() {
        let (proof, _, _) = inbound_fixture();
        let encoded = encode_sccp_native_inbound_message_proof_v1(&proof).unwrap();
        assert_eq!(
            decode_sccp_native_inbound_message_proof_v1(&[]),
            Err(SccpNativeAdmissionErrorV1::EmptyEncoding)
        );
        assert_eq!(
            decode_sccp_native_source_proof_envelope_v1(&[]),
            Err(SccpNativeAdmissionErrorV1::EmptyEncoding)
        );
        assert_eq!(
            SccpNativeAdmissionErrorV1::EmptyEncoding.to_string(),
            "encoded native proof is empty"
        );
        let truncated = &encoded[..encoded.len() - 1];
        assert_eq!(
            decode_sccp_native_inbound_message_proof_v1(truncated),
            Err(SccpNativeAdmissionErrorV1::InvalidNoritoEncoding)
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            decode_sccp_native_inbound_message_proof_v1(&trailing),
            Err(SccpNativeAdmissionErrorV1::InvalidNoritoEncoding)
        );
        let mut compressed = encoded.clone();
        compressed[NORITO_COMPRESSION_OFFSET] = 1;
        assert_eq!(
            decode_sccp_native_inbound_message_proof_v1(&compressed),
            Err(SccpNativeAdmissionErrorV1::InvalidNoritoEncoding)
        );
        let mut declared_bomb = encoded;
        declared_bomb[NORITO_LENGTH_OFFSET..NORITO_LENGTH_OFFSET + 8]
            .copy_from_slice(&(SCCP_NATIVE_ADMISSION_MAX_ENCODED_BYTES_U64_V1 + 1).to_le_bytes());
        assert!(matches!(
            decode_sccp_native_inbound_message_proof_v1(&declared_bomb),
            Err(SccpNativeAdmissionErrorV1::EncodedSize { .. })
        ));
        assert!(matches!(
            decode_sccp_native_inbound_message_proof_v1(&vec![
                0;
                SCCP_NATIVE_ADMISSION_MAX_ENCODED_BYTES_V1
                    + 1
            ]),
            Err(SccpNativeAdmissionErrorV1::EncodedSize { .. })
        ));
    }
    #[test]
    fn strict_json_rejects_unknown_variant_field_alias_order_and_trailing() {
        let (proof, _, _) = inbound_fixture();
        let json = norito::json::to_json(&proof).unwrap();
        assert_eq!(
            decode_sccp_native_inbound_message_proof_json_v1(""),
            Err(SccpNativeAdmissionErrorV1::EmptyEncoding)
        );
        assert_eq!(
            decode_sccp_native_source_proof_envelope_json_v1(""),
            Err(SccpNativeAdmissionErrorV1::EmptyEncoding)
        );
        let unknown_variant = json.replace("ethereum_beacon", "unknown_native_backend");
        assert_ne!(unknown_variant, json);
        assert_eq!(
            decode_sccp_native_inbound_message_proof_json_v1(&unknown_variant),
            Err(SccpNativeAdmissionErrorV1::InvalidJsonEncoding)
        );
        let unknown_field = format!("{{\"unknown\":0,{}", &json[1..]);
        assert_eq!(
            decode_sccp_native_inbound_message_proof_json_v1(&unknown_field),
            Err(SccpNativeAdmissionErrorV1::InvalidJsonEncoding)
        );
        let numeric_alias = json.replace("\"nonce\":\"11\"", "\"nonce\":11");
        assert_ne!(numeric_alias, json);
        assert_eq!(
            decode_sccp_native_inbound_message_proof_json_v1(&numeric_alias),
            Err(SccpNativeAdmissionErrorV1::InvalidJsonEncoding)
        );
        let without_first = json.replacen("{\"version\":1,\"payload\":", "{\"payload\":", 1);
        let reordered = without_first.replacen(",\"source\":", ",\"version\":1,\"source\":", 1);
        assert_ne!(reordered, json);
        assert_eq!(
            decode_sccp_native_inbound_message_proof_json_v1(&reordered),
            Err(SccpNativeAdmissionErrorV1::InvalidJsonEncoding)
        );
        assert_eq!(
            decode_sccp_native_inbound_message_proof_json_v1(&format!(" {json}")),
            Err(SccpNativeAdmissionErrorV1::InvalidJsonEncoding)
        );
        assert_eq!(
            decode_sccp_native_inbound_message_proof_json_v1(&format!("{json} true")),
            Err(SccpNativeAdmissionErrorV1::InvalidJsonEncoding)
        );
    }
    #[test]
    fn lane_backend_identity_and_anchor_substitutions_fail_closed() {
        let (proof, identity, trust_anchor) = inbound_fixture();
        let mut wrong_backend = proof.clone();
        wrong_backend.source.trust_anchor.backend = BridgeNativeProofBackendV1::BscParlia;
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(
                &wrong_backend,
                &identity,
                wrong_backend.source.trust_anchor,
            ),
            Err(SccpNativeAdmissionErrorV1::BackendMismatch)
        );
        let mut cross_sora = proof.clone();
        cross_sora.source.lane.source = SccpNetworkV1::SoraTaira;
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&cross_sora, &identity, trust_anchor),
            Err(SccpNativeAdmissionErrorV1::InvalidLane)
        );
        let mut cross_network = proof.clone();
        cross_network.source.lane.source = SccpNetworkV1::EthereumSepolia;
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&cross_network, &identity, trust_anchor),
            Err(SccpNativeAdmissionErrorV1::BackendMismatch)
        );
        let mut cross_family = proof.clone();
        cross_family.source.lane.source = SccpNetworkV1::BscTestnet;
        cross_family.source.trust_anchor.backend = BridgeNativeProofBackendV1::BscParlia;
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(
                &cross_family,
                &identity,
                cross_family.source.trust_anchor,
            ),
            Err(SccpNativeAdmissionErrorV1::BackendMismatch)
        );
        let mut wrong_identity = identity;
        let iroha_data_model::bridge::SccpSourceEmitterV1::Evm(emitter) =
            &mut wrong_identity.emitter
        else {
            panic!("fixture is EVM")
        };
        emitter.route_config_hash[0] ^= 1;
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&proof, &wrong_identity, trust_anchor,),
            Err(SccpNativeAdmissionErrorV1::SourceIdentityHashMismatch)
        );
        let wrong_anchor = SccpNativeTrustAnchorV1 {
            backend: trust_anchor.backend,
            checkpoint_height: trust_anchor.checkpoint_height,
            anchor_hash: [0xa7; 32],
        };
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&proof, &identity, wrong_anchor),
            Err(SccpNativeAdmissionErrorV1::TrustAnchorMismatch)
        );
        let wrong_height_anchor = SccpNativeTrustAnchorV1 {
            checkpoint_height: trust_anchor.checkpoint_height + 1,
            ..trust_anchor
        };
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&proof, &identity, wrong_height_anchor,),
            Err(SccpNativeAdmissionErrorV1::TrustAnchorMismatch)
        );
    }
    #[test]
    fn self_contained_source_finality_claim_is_exact_and_role_separated() {
        let (proof, identity, trust_anchor) = inbound_fixture();
        let validated =
            verify_sccp_native_inbound_message_proof_v1(&proof, &identity, trust_anchor)
                .expect("fixture native proof verifies");
        assert_eq!(validated.source_finality, proof.source.source_finality);
        let mut zero_height = proof.clone();
        zero_height.source.source_finality.height = 0;
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&zero_height, &identity, trust_anchor),
            Err(SccpNativeAdmissionErrorV1::HashRoleCollision)
        );
        let mut zero_hash = proof.clone();
        zero_hash.source.source_finality.block_hash = [0; 32];
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&zero_hash, &identity, trust_anchor),
            Err(SccpNativeAdmissionErrorV1::HashRoleCollision)
        );
        let mut collided_hash = proof.clone();
        collided_hash.source.source_finality.block_hash = collided_hash.source.payload_hash;
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&collided_hash, &identity, trust_anchor),
            Err(SccpNativeAdmissionErrorV1::HashRoleCollision)
        );
        let mut wrong_height = proof.clone();
        wrong_height.source.source_finality.height += 1;
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&wrong_height, &identity, trust_anchor),
            Err(SccpNativeAdmissionErrorV1::NormalizedResultMismatch(
                "source finality"
            ))
        );
        let mut wrong_hash = proof;
        wrong_hash.source.source_finality.block_hash[0] ^= 1;
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&wrong_hash, &identity, trust_anchor),
            Err(SccpNativeAdmissionErrorV1::NormalizedResultMismatch(
                "source finality"
            ))
        );
    }
    #[test]
    fn message_payload_digest_zero_and_collision_substitutions_fail_closed() {
        let (proof, identity, trust_anchor) = inbound_fixture();
        let mut wrong_message = proof.clone();
        let SccpPayloadV1::Transfer(payload) = &mut wrong_message.payload;
        payload.nonce += 1;
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&wrong_message, &identity, trust_anchor,),
            Err(SccpNativeAdmissionErrorV1::MessageIdMismatch)
        );
        let mut wrong_payload_lane = proof.clone();
        let SccpPayloadV1::Transfer(payload) = &mut wrong_payload_lane.payload;
        payload.source_domain = crate::SCCP_DOMAIN_BSC;
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(
                &wrong_payload_lane,
                &identity,
                trust_anchor,
            ),
            Err(SccpNativeAdmissionErrorV1::PayloadLaneMismatch)
        );
        let mut wrong_payload_hash = proof.clone();
        wrong_payload_hash.source.payload_hash = [0xb1; 32];
        ethereum_proof_mut(&mut wrong_payload_hash).payload_hash = [0xb1; 32];
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(
                &wrong_payload_hash,
                &identity,
                trust_anchor,
            ),
            Err(SccpNativeAdmissionErrorV1::PayloadHashMismatch)
        );
        let mut wrong_digest = proof.clone();
        wrong_digest.source.source_event_digest = [0xb2; 32];
        ethereum_proof_mut(&mut wrong_digest).source_event_digest = [0xb2; 32];
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&wrong_digest, &identity, trust_anchor),
            Err(SccpNativeAdmissionErrorV1::SourceEventDigestMismatch)
        );
        let mut zero_identity = proof.clone();
        zero_identity.source.source_identity_hash = [0; 32];
        ethereum_proof_mut(&mut zero_identity).source_identity_hash = [0; 32];
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&zero_identity, &identity, trust_anchor),
            Err(SccpNativeAdmissionErrorV1::HashRoleCollision)
        );
        let mut colliding_roles = proof.clone();
        colliding_roles.source.payload_hash = colliding_roles.source.message_id;
        let collision = colliding_roles.source.message_id;
        ethereum_proof_mut(&mut colliding_roles).payload_hash = collision;
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&colliding_roles, &identity, trust_anchor,),
            Err(SccpNativeAdmissionErrorV1::HashRoleCollision)
        );
    }
    #[test]
    fn oversized_nested_native_vector_reaches_family_bound_and_fails() {
        let (mut proof, identity, trust_anchor) = inbound_fixture();
        ethereum_proof_mut(&mut proof).receipt_proof.nodes = vec![vec![0x77; 1024 * 1024 + 1]];
        assert_eq!(
            verify_sccp_native_inbound_message_proof_v1(&proof, &identity, trust_anchor),
            Err(SccpNativeAdmissionErrorV1::Ethereum(
                EthereumNativeSourceErrorV1::MptProofBounds(EthereumNativeMptRoleV1::Receipt)
            ))
        );
    }
    #[test]
    fn native_backend_selection_is_closed_by_exact_network() {
        let mappings = [
            (
                SccpNetworkV1::EthereumMainnet,
                BridgeNativeProofBackendV1::EthereumBeacon,
            ),
            (
                SccpNetworkV1::EthereumSepolia,
                BridgeNativeProofBackendV1::EthereumBeacon,
            ),
            (
                SccpNetworkV1::BscMainnet,
                BridgeNativeProofBackendV1::BscParlia,
            ),
            (
                SccpNetworkV1::BscTestnet,
                BridgeNativeProofBackendV1::BscParlia,
            ),
            (
                SccpNetworkV1::TronMainnet,
                BridgeNativeProofBackendV1::TronDpos,
            ),
            (
                SccpNetworkV1::TronNile,
                BridgeNativeProofBackendV1::TronDpos,
            ),
            (
                SccpNetworkV1::TronShasta,
                BridgeNativeProofBackendV1::TronDpos,
            ),
        ];
        for (network, backend) in mappings {
            assert_eq!(
                sccp_native_backend_for_source_network_v1(network),
                Some(backend)
            );
        }
        assert_eq!(
            sccp_native_backend_for_source_network_v1(SccpNetworkV1::SoraTaira),
            None
        );
    }
}
