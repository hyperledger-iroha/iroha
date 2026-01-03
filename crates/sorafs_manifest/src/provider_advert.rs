//! Norito-encoded provider advertisements for the SoraFS node ↔ client protocol.
//!
//! The advertisement body is signed by governed providers and propagated
//! through the discovery mesh. TTLs are capped at 24 hours with clients
//! refreshing half-way through the validity window to avoid stale routes.

use core::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use norito::{
    core::{DecodeFromSlice, decode_field_canonical},
    derive::{JsonSerialize, NoritoDeserialize, NoritoSerialize},
    json::{FastJsonWrite, JsonSerialize as NoritoJsonSerialize},
};
use thiserror::Error;

use crate::chunker_registry;

/// Advertisement schema version.
pub const PROVIDER_ADVERT_VERSION_V1: u8 = 1;

/// Maximum advertisement time-to-live (seconds).
pub const MAX_ADVERT_TTL_SECS: u64 = 24 * 60 * 60;

/// Recommended refresh interval (seconds) == 12 hours.
pub const REFRESH_RECOMMENDATION_SECS: u64 = 12 * 60 * 60;

/// Norito payload advertised by storage providers.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct ProviderAdvertV1 {
    /// Version identifier; must equal [`PROVIDER_ADVERT_VERSION_V1`].
    pub version: u8,
    /// Unix timestamp (seconds) when the advert was issued.
    pub issued_at: u64,
    /// Unix timestamp (seconds) when the advert expires.
    pub expires_at: u64,
    /// Signed body of the advertisement.
    pub body: ProviderAdvertBodyV1,
    /// Provider signature covering the serialized body.
    pub signature: AdvertSignature,
    /// Whether consumers should enforce signature validation.
    pub signature_strict: bool,
    /// Allow consumers to ignore unknown capability TLVs (GREASE-friendly).
    #[norito(default)]
    pub allow_unknown_capabilities: bool,
}

impl ProviderAdvertV1 {
    /// Returns the advert TTL in seconds.
    #[must_use]
    pub fn ttl(&self) -> u64 {
        self.expires_at.saturating_sub(self.issued_at)
    }

    /// Returns the recommended refresh deadline (`issued_at + min(ceil(TTL/2), 12h)`).
    #[must_use]
    pub fn refresh_deadline(&self) -> u64 {
        let ttl = self.ttl();
        let half_ttl = ttl.div_ceil(2);
        let refresh_offset = half_ttl.min(REFRESH_RECOMMENDATION_SECS);
        self.issued_at.saturating_add(refresh_offset)
    }

    /// Validates timestamps, TTL, and required body fields.
    pub fn validate(&self, now: u64) -> Result<(), AdvertValidationError> {
        if self.version != PROVIDER_ADVERT_VERSION_V1 {
            return Err(AdvertValidationError::UnsupportedVersion(self.version));
        }
        if self.expires_at <= self.issued_at {
            return Err(AdvertValidationError::InvalidTimestamps);
        }
        let ttl = self.ttl();
        if ttl == 0 || ttl > MAX_ADVERT_TTL_SECS {
            return Err(AdvertValidationError::TtlOutOfRange {
                ttl,
                max: MAX_ADVERT_TTL_SECS,
            });
        }
        if now > self.expires_at {
            return Err(AdvertValidationError::Expired {
                now,
                expires_at: self.expires_at,
            });
        }
        if self.body.endpoints.is_empty() {
            return Err(AdvertValidationError::MissingEndpoints);
        }
        if self.body.rendezvous_topics.is_empty() {
            return Err(AdvertValidationError::MissingRendezvousTopics);
        }
        if self.body.capabilities.is_empty() {
            return Err(AdvertValidationError::MissingCapabilities);
        }
        if self.body.path_policy.min_guard_weight == 0 {
            return Err(AdvertValidationError::InvalidPathPolicy);
        }
        Ok(())
    }
}

/// Provider advertisement body signed by the provider.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct ProviderAdvertBodyV1 {
    /// Governance-controlled provider identifier (32-byte digest).
    pub provider_id: [u8; 32],
    /// SoraFS chunking profile advertised by the provider.
    pub profile_id: String,
    /// Additional handles recognised when negotiating chunkers.
    #[norito(default)]
    pub profile_aliases: Option<Vec<String>>,
    /// Stake pointer used for admission and weight calculations.
    pub stake: StakePointer,
    /// Quality-of-service hints for routing decisions.
    pub qos: QosHints,
    /// Capability TLVs advertised by the provider.
    pub capabilities: Vec<CapabilityTlv>,
    /// Service endpoints (gateway or direct gRPC hosts).
    pub endpoints: Vec<AdvertEndpoint>,
    /// Rendezvous topics published via the discovery mesh.
    pub rendezvous_topics: Vec<RendezvousTopic>,
    /// Path diversity constraints to mitigate eclipse attacks.
    pub path_policy: PathDiversityPolicy,
    /// Optional notes for operators or diagnostics.
    pub notes: Option<String>,
    /// Optional stream budget advertised by the provider.
    #[norito(default)]
    pub stream_budget: Option<StreamBudgetV1>,
    /// Optional transport hints indicating supported protocols.
    #[norito(default)]
    pub transport_hints: Option<Vec<TransportHintV1>>,
}

/// Stake pointer encoded in the advertisement.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct StakePointer {
    /// Identifier of the staking pool.
    pub pool_id: [u8; 32],
    /// Amount staked in minor units (scaled XOR).
    pub stake_amount: u128,
}

impl StakePointer {
    /// Returns true if the stake is non-zero.
    #[must_use]
    pub fn is_positive(&self) -> bool {
        self.stake_amount > 0
    }
}

/// QoS hints used by clients to pick storage providers.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct QosHints {
    /// Availability class advertised by the provider.
    pub availability: AvailabilityTier,
    /// Maximum retrieval latency target (milliseconds).
    pub max_retrieval_latency_ms: u32,
    /// Maximum concurrent streams guaranteed.
    pub max_concurrent_streams: u16,
}

impl QosHints {
    /// Ensures the QoS configuration is internally consistent.
    pub fn validate(&self) -> Result<(), AdvertValidationError> {
        if self.max_retrieval_latency_ms == 0 {
            return Err(AdvertValidationError::InvalidQos);
        }
        if self.max_concurrent_streams == 0 {
            return Err(AdvertValidationError::InvalidQos);
        }
        Ok(())
    }
}

/// Availability tier definitions.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum AvailabilityTier {
    /// Hot availability — sub-second retrieval targets.
    Hot = 1,
    /// Warm availability — under one minute cold-start.
    Warm = 2,
    /// Cold availability — archival with relaxed SLA.
    Cold = 3,
}

/// Capability TLV advertised by a provider.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct CapabilityTlv {
    /// Capability type identifier.
    pub cap_type: CapabilityType,
    /// Capability payload, encoded as raw bytes.
    pub payload: Vec<u8>,
}

/// Enumerates high-level capability families.
#[derive(
    Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[repr(u16)]
pub enum CapabilityType {
    /// Provider supports SoraFS chunk retrieval over Torii.
    ToriiGateway = 0x0001,
    /// Provider exposes QUIC retrieval with Noise handshake.
    QuicNoise = 0x0002,
    /// Provider supports ranged chunk requests for multi-source fetch.
    ChunkRangeFetch = 0x0004,
    /// Provider advertises hybrid SoraNet PQ support (stage flags).
    SoraNetHybridPq = 0x0005,
    /// Custom capability encoded via payload.
    VendorReserved = 0xFF00,
}

/// Payload describing range-fetch capability metadata.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct ProviderCapabilityRangeV1 {
    /// Maximum contiguous chunk span that may be served per request.
    pub max_chunk_span: u32,
    /// Minimum granularity (bytes) supported when seeking within a chunk.
    pub min_granularity: u32,
    /// Whether sparse (non-contiguous) offsets are supported.
    pub supports_sparse_offsets: bool,
    /// Whether requests must align to `min_granularity`.
    pub requires_alignment: bool,
    /// Whether Merkle proofs can accompany ranged responses.
    pub supports_merkle_proof: bool,
}

impl Default for ProviderCapabilityRangeV1 {
    fn default() -> Self {
        Self {
            max_chunk_span: 1,
            min_granularity: 1,
            supports_sparse_offsets: false,
            requires_alignment: false,
            supports_merkle_proof: false,
        }
    }
}

const PQ_FLAG_GUARD: u8 = 0x01;
const PQ_FLAG_MAJORITY: u8 = 0x02;
const PQ_FLAG_STRICT: u8 = 0x04;

/// Errors raised when validating SoraNet PQ capability payloads.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PqCapabilityError {
    #[error("soranet_pq capability payload must be exactly 1 byte, found {0}")]
    InvalidLength(usize),
    #[error("soranet_pq capability must advertise guard-level support")]
    MissingGuardSupport,
    #[error("soranet_pq capability marked strict support without majority support")]
    StrictWithoutMajority,
}

/// Bitflag payload describing SoraNet PQ support levels.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct ProviderCapabilitySoranetPqV1 {
    pub supports_guard: bool,
    pub supports_majority: bool,
    pub supports_strict: bool,
}

impl ProviderCapabilitySoranetPqV1 {
    /// Validate invariants for the payload.
    pub fn validate(&self) -> Result<(), PqCapabilityError> {
        if !self.supports_guard {
            return Err(PqCapabilityError::MissingGuardSupport);
        }
        if self.supports_strict && !self.supports_majority {
            return Err(PqCapabilityError::StrictWithoutMajority);
        }
        Ok(())
    }

    /// Encode the payload into the compact bitflag representation.
    pub fn to_bytes(self) -> Result<Vec<u8>, PqCapabilityError> {
        self.validate()?;
        let mut mask = 0u8;
        if self.supports_guard {
            mask |= PQ_FLAG_GUARD;
        }
        if self.supports_majority {
            mask |= PQ_FLAG_MAJORITY;
        }
        if self.supports_strict {
            mask |= PQ_FLAG_STRICT;
        }
        Ok(vec![mask])
    }

    /// Decode the payload from the bitflag representation.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PqCapabilityError> {
        if bytes.len() != 1 {
            return Err(PqCapabilityError::InvalidLength(bytes.len()));
        }
        let mask = bytes[0];
        let supports_guard = (mask & PQ_FLAG_GUARD) != 0;
        let supports_majority = (mask & PQ_FLAG_MAJORITY) != 0;
        let supports_strict = (mask & PQ_FLAG_STRICT) != 0;
        let payload = Self {
            supports_guard,
            supports_majority,
            supports_strict,
        };
        payload.validate()?;
        Ok(payload)
    }
}

impl ProviderCapabilityRangeV1 {
    /// Validates the capability metadata.
    pub fn validate(&self) -> Result<(), RangeCapabilityError> {
        if self.max_chunk_span == 0 {
            return Err(RangeCapabilityError::InvalidMaxChunkSpan);
        }
        if self.min_granularity == 0 {
            return Err(RangeCapabilityError::InvalidMinGranularity);
        }
        if self.min_granularity > self.max_chunk_span {
            return Err(RangeCapabilityError::GranularityExceedsSpan {
                granularity: self.min_granularity,
                span: self.max_chunk_span,
            });
        }
        Ok(())
    }

    /// Encodes the capability metadata to the compact 9-byte TLV payload.
    pub fn to_bytes(self) -> Result<Vec<u8>, RangeCapabilityError> {
        self.validate()?;
        let mut bytes = Vec::with_capacity(9);
        bytes.extend_from_slice(&self.max_chunk_span.to_le_bytes());
        bytes.extend_from_slice(&self.min_granularity.to_le_bytes());
        let mut flags = 0u8;
        if self.supports_sparse_offsets {
            flags |= 0x01;
        }
        if self.requires_alignment {
            flags |= 0x02;
        }
        if self.supports_merkle_proof {
            flags |= 0x04;
        }
        bytes.push(flags);
        Ok(bytes)
    }

    /// Decodes metadata from the compact TLV payload.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, RangeCapabilityError> {
        if bytes.len() != 9 {
            return Err(RangeCapabilityError::Decode(format!(
                "expected 9 bytes, got {}",
                bytes.len()
            )));
        }
        let max_chunk_span = u32::from_le_bytes(bytes[0..4].try_into().expect("length checked"));
        let min_granularity = u32::from_le_bytes(bytes[4..8].try_into().expect("length checked"));
        let flags = bytes[8];
        let value = Self {
            max_chunk_span,
            min_granularity,
            supports_sparse_offsets: flags & 0x01 != 0,
            requires_alignment: flags & 0x02 != 0,
            supports_merkle_proof: flags & 0x04 != 0,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Advertised stream budget for ranged fetches.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct StreamBudgetV1 {
    /// Maximum concurrent ranged fetches the provider will serve.
    pub max_in_flight: u16,
    /// Sustained byte rate (bytes per second) reserved for ranged traffic.
    pub max_bytes_per_sec: u64,
    /// Optional burst allowance (bytes).
    #[norito(default)]
    pub burst_bytes: Option<u64>,
}

impl<'a> DecodeFromSlice<'a> for StreamBudgetV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        decode_field_canonical::<Self>(bytes)
    }
}

impl StreamBudgetV1 {
    /// Validates the stream budget configuration.
    pub fn validate(&self) -> Result<(), StreamBudgetError> {
        if self.max_in_flight == 0 {
            return Err(StreamBudgetError::InvalidMaxInFlight);
        }
        if self.max_bytes_per_sec == 0 {
            return Err(StreamBudgetError::InvalidMaxBytesPerSec);
        }
        if let Some(burst) = self.burst_bytes {
            if burst == 0 {
                return Err(StreamBudgetError::InvalidBurstBytes);
            }
            if burst > self.max_bytes_per_sec {
                return Err(StreamBudgetError::BurstExceedsRate {
                    burst,
                    rate: self.max_bytes_per_sec,
                });
            }
        }
        Ok(())
    }
}

/// Hint describing a supported ranged-fetch transport.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct TransportHintV1 {
    /// Transport protocol identifier.
    pub protocol: TransportProtocol,
    /// Preference ordering (lower values are preferred).
    pub priority: u8,
}

impl TransportHintV1 {
    /// Ensures the transport hint is internally consistent.
    pub fn validate(&self) -> Result<(), TransportHintError> {
        if self.priority > 15 {
            return Err(TransportHintError::InvalidPriority);
        }
        Ok(())
    }
}

impl<'a> DecodeFromSlice<'a> for TransportHintV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (protocol_raw, used_protocol) = <u8 as DecodeFromSlice>::decode_from_slice(bytes)?;
        let protocol = TransportProtocol::from_u8(protocol_raw).ok_or_else(|| {
            norito::core::Error::Message(format!("unknown transport protocol {protocol_raw}"))
        })?;
        let (priority, used_priority) =
            <u8 as DecodeFromSlice>::decode_from_slice(&bytes[used_protocol..])?;
        Ok((Self { protocol, priority }, used_protocol + used_priority))
    }
}

/// Transport protocols supported by providers.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TransportProtocol {
    /// HTTP range requests served via Torii.
    ToriiHttpRange = 1,
    /// QUIC stream delivery.
    QuicStream = 2,
    /// SoraNet relay transport.
    SoraNetRelay = 3,
    /// Vendor-reserved protocol identifier.
    VendorReserved = 255,
}

impl TransportProtocol {
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::ToriiHttpRange),
            2 => Some(Self::QuicStream),
            3 => Some(Self::SoraNetRelay),
            255 => Some(Self::VendorReserved),
            _ => None,
        }
    }
}

/// Errors raised when validating range capability metadata.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RangeCapabilityError {
    #[error("max_chunk_span must be positive")]
    InvalidMaxChunkSpan,
    #[error("min_granularity must be positive")]
    InvalidMinGranularity,
    #[error("min_granularity {granularity} exceeds max_chunk_span {span}")]
    GranularityExceedsSpan { granularity: u32, span: u32 },
    #[error("failed to encode capability payload: {0}")]
    Encode(String),
    #[error("failed to decode capability payload: {0}")]
    Decode(String),
}

/// Errors raised when validating stream budgets.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum StreamBudgetError {
    #[error("max_in_flight must be at least 1")]
    InvalidMaxInFlight,
    #[error("max_bytes_per_sec must be positive")]
    InvalidMaxBytesPerSec,
    #[error("burst_bytes must be positive")]
    InvalidBurstBytes,
    #[error("burst_bytes {burst} must be <= max_bytes_per_sec {rate}")]
    BurstExceedsRate { burst: u64, rate: u64 },
}

/// Errors raised when validating transport hints.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum TransportHintError {
    #[error("transport hint priority is out of range")]
    InvalidPriority,
}

/// Service endpoint exposed by the provider.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct AdvertEndpoint {
    /// Logical endpoint type.
    pub kind: EndpointKind,
    /// Host pattern (FQDN or CIDR notation) the endpoint serves.
    pub host_pattern: String,
    /// Optional TLS fingerprint / ALPN hints.
    pub metadata: Vec<EndpointMetadata>,
}

/// Endpoint metadata TLV fields.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct EndpointMetadata {
    /// Metadata field identifier.
    pub key: EndpointMetadataKey,
    /// Raw value bytes.
    pub value: Vec<u8>,
}

/// Metadata keys for endpoint hints.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[repr(u16)]
pub enum EndpointMetadataKey {
    /// TLS certificate fingerprint (SHA-256).
    TlsFingerprint = 0x0001,
    /// Supported ALPN identifier.
    Alpn = 0x0002,
    /// Region tag for routing hints.
    Region = 0x0003,
}

/// Endpoint kind enumeration.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum EndpointKind {
    /// Torii HTTP/2 gateway.
    Torii = 1,
    /// Direct QUIC retrieval.
    Quic = 2,
    /// Norito-RPC streaming endpoint.
    NoritoRpc = 3,
}

/// Rendezvous topic advertised for discovery.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct RendezvousTopic {
    /// Topic identifier (e.g., `sorafs.sf1.primary`).
    pub topic: String,
    /// Region or locale code (ISO-3166 alpha-2 or `global`).
    pub region: String,
}

/// Path diversity policy to mitigate eclipse attacks.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct PathDiversityPolicy {
    /// Minimum guard weight (stake percentile) allowed per path.
    pub min_guard_weight: u16,
    /// Maximum providers from the same ASN allowed in a circuit.
    pub max_same_asn_per_path: u8,
    /// Maximum entries from the same staking pool.
    pub max_same_pool_per_path: u8,
}

/// Signature covering the serialized advertisement body.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, PartialEq, Eq)]
#[norito(decode_from_slice)]
pub struct AdvertSignature {
    /// Signature algorithm identifier.
    pub algorithm: SignatureAlgorithm,
    /// Public key bytes.
    pub public_key: Vec<u8>,
    /// Raw signature bytes.
    pub signature: Vec<u8>,
}

/// Supported advertisement signature algorithms.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum SignatureAlgorithm {
    /// Ed25519 signature (preferred).
    Ed25519 = 1,
    /// Multi-signature aggregated via Norito (reserved).
    MultiSig = 2,
}

impl FastJsonWrite for SignatureAlgorithm {
    fn write_json(&self, out: &mut String) {
        let label = match self {
            SignatureAlgorithm::Ed25519 => "ed25519",
            SignatureAlgorithm::MultiSig => "multi-sig",
        };
        NoritoJsonSerialize::json_serialize(&label, out);
    }
}

/// Builder for constructing provider advertisements.
#[derive(Debug, Default)]
pub struct ProviderAdvertBuilder {
    profile_id: Option<String>,
    profile_aliases: Option<Vec<String>>,
    provider_id: Option<[u8; 32]>,
    stake_pool_id: Option<[u8; 32]>,
    stake_amount: Option<u128>,
    availability: Option<AvailabilityTier>,
    max_latency_ms: Option<u32>,
    max_streams: Option<u16>,
    capabilities: Vec<CapabilityTlv>,
    endpoints: Vec<AdvertEndpoint>,
    topics: Vec<RendezvousTopic>,
    min_guard_weight: Option<u16>,
    max_same_asn: Option<u8>,
    max_same_pool: Option<u8>,
    notes: Option<String>,
    stream_budget: Option<StreamBudgetV1>,
    transport_hints: Vec<TransportHintV1>,
    issued_at: Option<u64>,
    ttl_secs: Option<u64>,
    signature_alg: Option<SignatureAlgorithm>,
    public_key: Option<Vec<u8>>,
    signature: Option<Vec<u8>>,
    allow_unknown_capabilities: bool,
}

/// Errors raised while building provider adverts.
#[derive(Debug, Error)]
pub enum ProviderAdvertBuildError {
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("provider advert validation failed: {0}")]
    Validation(AdvertValidationError),
}

impl ProviderAdvertBuilder {
    /// Creates a new, empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn profile_id(&mut self, profile_id: impl Into<String>) -> &mut Self {
        self.profile_id = Some(profile_id.into());
        self
    }

    #[must_use]
    pub fn profile_aliases(&mut self, aliases: Vec<String>) -> &mut Self {
        self.profile_aliases = Some(aliases);
        self
    }

    #[must_use]
    pub fn provider_id(&mut self, provider_id: [u8; 32]) -> &mut Self {
        self.provider_id = Some(provider_id);
        self
    }

    #[must_use]
    pub fn stake_pool_id(&mut self, stake_pool_id: [u8; 32]) -> &mut Self {
        self.stake_pool_id = Some(stake_pool_id);
        self
    }

    #[must_use]
    pub fn stake_amount(&mut self, stake_amount: u128) -> &mut Self {
        self.stake_amount = Some(stake_amount);
        self
    }

    #[must_use]
    pub fn availability(&mut self, availability: AvailabilityTier) -> &mut Self {
        self.availability = Some(availability);
        self
    }

    #[must_use]
    pub fn max_retrieval_latency_ms(&mut self, latency: u32) -> &mut Self {
        self.max_latency_ms = Some(latency);
        self
    }

    #[must_use]
    pub fn max_concurrent_streams(&mut self, streams: u16) -> &mut Self {
        self.max_streams = Some(streams);
        self
    }

    #[must_use]
    pub fn add_capability(&mut self, capability: CapabilityTlv) -> &mut Self {
        self.capabilities.push(capability);
        self
    }

    pub fn add_range_capability(
        &mut self,
        capability: ProviderCapabilityRangeV1,
    ) -> Result<&mut Self, RangeCapabilityError> {
        let payload = capability.to_bytes()?;
        self.capabilities
            .retain(|cap| cap.cap_type != CapabilityType::ChunkRangeFetch);
        self.capabilities.push(CapabilityTlv {
            cap_type: CapabilityType::ChunkRangeFetch,
            payload,
        });
        Ok(self)
    }

    #[must_use]
    pub fn add_endpoint(&mut self, endpoint: AdvertEndpoint) -> &mut Self {
        self.endpoints.push(endpoint);
        self
    }

    #[must_use]
    pub fn add_topic(&mut self, topic: RendezvousTopic) -> &mut Self {
        self.topics.push(topic);
        self
    }

    #[must_use]
    pub fn allow_unknown_capabilities(&mut self, allow: bool) -> &mut Self {
        self.allow_unknown_capabilities = allow;
        self
    }

    #[must_use]
    pub fn path_policy_min_guard_weight(&mut self, weight: u16) -> &mut Self {
        self.min_guard_weight = Some(weight);
        self
    }

    #[must_use]
    pub fn path_policy_max_same_asn_per_path(&mut self, limit: u8) -> &mut Self {
        self.max_same_asn = Some(limit);
        self
    }

    #[must_use]
    pub fn path_policy_max_same_pool_per_path(&mut self, limit: u8) -> &mut Self {
        self.max_same_pool = Some(limit);
        self
    }

    #[must_use]
    pub fn notes(&mut self, notes: impl Into<String>) -> &mut Self {
        self.notes = Some(notes.into());
        self
    }

    #[must_use]
    pub fn stream_budget(&mut self, budget: StreamBudgetV1) -> &mut Self {
        self.stream_budget = Some(budget);
        self
    }

    #[must_use]
    pub fn clear_stream_budget(&mut self) -> &mut Self {
        self.stream_budget = None;
        self
    }

    #[must_use]
    pub fn transport_hints(&mut self, hints: Vec<TransportHintV1>) -> &mut Self {
        self.transport_hints = hints;
        self
    }

    #[must_use]
    pub fn add_transport_hint(&mut self, hint: TransportHintV1) -> &mut Self {
        self.transport_hints.push(hint);
        self
    }

    #[must_use]
    pub fn issued_at(&mut self, issued_at: u64) -> &mut Self {
        self.issued_at = Some(issued_at);
        self
    }

    #[must_use]
    pub fn ttl_secs(&mut self, ttl: u64) -> &mut Self {
        self.ttl_secs = Some(ttl);
        self
    }

    #[must_use]
    pub fn signature(
        &mut self,
        algorithm: SignatureAlgorithm,
        public_key: Vec<u8>,
        signature: Vec<u8>,
    ) -> &mut Self {
        self.signature_alg = Some(algorithm);
        self.public_key = Some(public_key);
        self.signature = Some(signature);
        self
    }

    /// Consumes the builder and returns a fully validated advert.
    pub fn build(self) -> Result<ProviderAdvertV1, ProviderAdvertBuildError> {
        let requested_profile = self
            .profile_id
            .ok_or(ProviderAdvertBuildError::MissingField("profile_id"))?;
        let descriptor = chunker_registry::lookup_by_handle(&requested_profile).ok_or(
            ProviderAdvertBuildError::Validation(AdvertValidationError::UnknownProfileHandle {
                handle: requested_profile.clone(),
            }),
        )?;
        let canonical_profile = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let profile_id = canonical_profile.clone();
        let provider_id = self
            .provider_id
            .ok_or(ProviderAdvertBuildError::MissingField("provider_id"))?;
        let stake_pool_id = self
            .stake_pool_id
            .ok_or(ProviderAdvertBuildError::MissingField("stake_pool_id"))?;
        let stake_amount = self
            .stake_amount
            .ok_or(ProviderAdvertBuildError::MissingField("stake_amount"))?;
        let availability = self
            .availability
            .ok_or(ProviderAdvertBuildError::MissingField("availability"))?;
        let max_latency_ms = self
            .max_latency_ms
            .ok_or(ProviderAdvertBuildError::MissingField(
                "max_retrieval_latency_ms",
            ))?;
        let max_streams = self
            .max_streams
            .ok_or(ProviderAdvertBuildError::MissingField(
                "max_concurrent_streams",
            ))?;
        if self.capabilities.is_empty() {
            return Err(ProviderAdvertBuildError::MissingField("capabilities"));
        }
        if self.endpoints.is_empty() {
            return Err(ProviderAdvertBuildError::MissingField("endpoints"));
        }
        if self.topics.is_empty() {
            return Err(ProviderAdvertBuildError::MissingField("rendezvous_topics"));
        }
        let signature_alg = self.signature_alg.unwrap_or(SignatureAlgorithm::Ed25519);
        let public_key = self
            .public_key
            .ok_or(ProviderAdvertBuildError::MissingField("public_key"))?;
        let signature = self
            .signature
            .ok_or(ProviderAdvertBuildError::MissingField("signature"))?;

        let issued_at = self
            .issued_at
            .unwrap_or_else(|| unix_time_now().unwrap_or(0));
        let ttl = self
            .ttl_secs
            .unwrap_or(REFRESH_RECOMMENDATION_SECS * 2)
            .min(MAX_ADVERT_TTL_SECS);
        let expires_at = issued_at
            .checked_add(ttl)
            .ok_or(ProviderAdvertBuildError::Validation(
                AdvertValidationError::InvalidTimestamps,
            ))?;

        if ttl == 0 {
            return Err(ProviderAdvertBuildError::Validation(
                AdvertValidationError::InvalidTimestamps,
            ));
        }

        let profile_aliases = self.profile_aliases.map(|mut aliases| {
            if !aliases.iter().any(|alias| alias == &profile_id) {
                aliases.insert(0, profile_id.clone());
            } else if !aliases
                .first()
                .map(|alias| alias == &profile_id)
                .unwrap_or(false)
            {
                aliases.retain(|alias| alias != &profile_id);
                aliases.insert(0, profile_id.clone());
            }
            aliases
        });

        let stream_budget = self.stream_budget;
        let transport_hints = if self.transport_hints.is_empty() {
            None
        } else {
            Some(self.transport_hints.clone())
        };

        let body = ProviderAdvertBodyV1 {
            provider_id,
            profile_id,
            profile_aliases,
            stake: StakePointer {
                pool_id: stake_pool_id,
                stake_amount,
            },
            qos: QosHints {
                availability,
                max_retrieval_latency_ms: max_latency_ms,
                max_concurrent_streams: max_streams,
            },
            capabilities: self.capabilities,
            endpoints: self.endpoints,
            rendezvous_topics: self.topics,
            path_policy: PathDiversityPolicy {
                min_guard_weight: self.min_guard_weight.unwrap_or(10),
                max_same_asn_per_path: self.max_same_asn.unwrap_or(1),
                max_same_pool_per_path: self.max_same_pool.unwrap_or(1),
            },
            notes: self.notes,
            stream_budget,
            transport_hints,
        };

        body.validate()
            .map_err(ProviderAdvertBuildError::Validation)?;

        let advert = ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            issued_at,
            expires_at,
            body,
            signature: AdvertSignature {
                algorithm: signature_alg,
                public_key,
                signature,
            },
            signature_strict: true,
            allow_unknown_capabilities: self.allow_unknown_capabilities,
        };

        advert
            .validate_with_body(issued_at)
            .map_err(ProviderAdvertBuildError::Validation)?;
        Ok(advert)
    }
}

impl ProviderAdvertV1 {
    /// Returns a builder for the advert.
    #[must_use]
    pub fn builder() -> ProviderAdvertBuilder {
        ProviderAdvertBuilder::new()
    }
}

/// Errors raised while validating a provider advert.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdvertValidationError {
    #[error("unsupported provider advert version: {0}")]
    UnsupportedVersion(u8),
    #[error("expires_at must be greater than issued_at")]
    InvalidTimestamps,
    #[error("advert TTL {ttl} exceeds maximum {max}")]
    TtlOutOfRange { ttl: u64, max: u64 },
    #[error("advert expired (now={now}, expires_at={expires_at})")]
    Expired { now: u64, expires_at: u64 },
    #[error("provider advert must advertise at least one endpoint")]
    MissingEndpoints,
    #[error("provider advert must include at least one rendezvous topic")]
    MissingRendezvousTopics,
    #[error("provider advert must advertise at least one capability")]
    MissingCapabilities,
    #[error("path diversity policy must require a positive guard weight")]
    InvalidPathPolicy,
    #[error("invalid QoS configuration")]
    InvalidQos,
    #[error("profile_aliases must include the advertised chunker handle and canonical aliases")]
    MissingProfileAliases,
    #[error("profile_aliases may not contain empty or whitespace-only entries")]
    InvalidProfileAlias,
    #[error("profile_aliases must include required alias {alias}")]
    MissingRequiredAlias { alias: String },
    #[error("unknown chunker profile handle advertised: {handle}")]
    UnknownProfileHandle { handle: String },
    #[error("duplicate profile alias detected: {alias}")]
    DuplicateProfileAlias { alias: String },
    #[error("range capability payload missing or malformed")]
    InvalidRangeCapabilityPayload,
    #[error("range capability payload invalid: {0}")]
    InvalidRangeCapability(RangeCapabilityError),
    #[error("duplicate range capability TLV detected")]
    DuplicateRangeCapability,
    #[error("stream budget or transport hints require chunk_range_fetch capability")]
    RangeMetadataWithoutCapability,
    #[error("stream budget invalid: {0}")]
    InvalidStreamBudget(StreamBudgetError),
    #[error("transport hints must not be empty when provided")]
    EmptyTransportHints,
    #[error("transport hints must have unique protocols")]
    DuplicateTransportProtocol,
    #[error("transport hint invalid: {0}")]
    InvalidTransportHint(TransportHintError),
    #[error("soranet transport hints require a soranet capability")]
    SoranetTransportWithoutCapability,
}

impl ProviderAdvertBodyV1 {
    /// Validates the body independently of outer metadata.
    pub fn validate(&self) -> Result<(), AdvertValidationError> {
        if !self.stake.is_positive() {
            return Err(AdvertValidationError::InvalidQos);
        }
        self.qos.validate()?;
        let descriptor = chunker_registry::lookup_by_handle(&self.profile_id).ok_or_else(|| {
            AdvertValidationError::UnknownProfileHandle {
                handle: self.profile_id.clone(),
            }
        })?;
        let aliases = self
            .profile_aliases
            .as_ref()
            .ok_or(AdvertValidationError::MissingProfileAliases)?;
        if aliases.is_empty() {
            return Err(AdvertValidationError::MissingProfileAliases);
        }
        let mut seen = std::collections::HashSet::new();
        for alias in aliases {
            let trimmed = alias.trim();
            if trimmed.is_empty() {
                return Err(AdvertValidationError::InvalidProfileAlias);
            }
            if !seen.insert(trimmed.to_owned()) {
                return Err(AdvertValidationError::DuplicateProfileAlias {
                    alias: trimmed.to_owned(),
                });
            }
        }
        for required in descriptor.aliases {
            if !seen.contains(*required) {
                return Err(AdvertValidationError::MissingRequiredAlias {
                    alias: (*required).to_owned(),
                });
            }
        }

        let mut seen_range_capability = false;
        for capability in &self.capabilities {
            if capability.cap_type == CapabilityType::ChunkRangeFetch {
                if seen_range_capability {
                    return Err(AdvertValidationError::DuplicateRangeCapability);
                }
                let range_cap = ProviderCapabilityRangeV1::from_bytes(&capability.payload)
                    .map_err(|_| AdvertValidationError::InvalidRangeCapabilityPayload)?;
                range_cap
                    .validate()
                    .map_err(AdvertValidationError::InvalidRangeCapability)?;
                seen_range_capability = true;
            }
        }

        let mut has_stream_budget = false;
        if let Some(budget) = &self.stream_budget {
            has_stream_budget = true;
            budget
                .validate()
                .map_err(AdvertValidationError::InvalidStreamBudget)?;
        }

        let mut has_transport_hints = false;
        if let Some(hints) = &self.transport_hints {
            if hints.is_empty() {
                return Err(AdvertValidationError::EmptyTransportHints);
            }
            has_transport_hints = true;
            let mut seen_protocols = std::collections::HashSet::new();
            for hint in hints {
                hint.validate()
                    .map_err(AdvertValidationError::InvalidTransportHint)?;
                if !seen_protocols.insert(hint.protocol) {
                    return Err(AdvertValidationError::DuplicateTransportProtocol);
                }
            }
            let has_soranet_hint = hints
                .iter()
                .any(|hint| hint.protocol == TransportProtocol::SoraNetRelay);
            if has_soranet_hint
                && !self
                    .capabilities
                    .iter()
                    .any(|cap| cap.cap_type == CapabilityType::SoraNetHybridPq)
            {
                return Err(AdvertValidationError::SoranetTransportWithoutCapability);
            }
        }

        if (has_stream_budget || has_transport_hints) && !seen_range_capability {
            return Err(AdvertValidationError::RangeMetadataWithoutCapability);
        }
        Ok(())
    }
}

impl ProviderAdvertV1 {
    /// Combined validation helper for outer advert and inner body.
    pub fn validate_with_body(&self, now: u64) -> Result<(), AdvertValidationError> {
        self.validate(now)?;
        self.body.validate()?;
        Ok(())
    }
}

/// Human-friendly accessors useful for monitoring dashboards.
impl ProviderAdvertV1 {
    /// Returns the TTL as a [`Duration`].
    #[must_use]
    pub fn ttl_duration(&self) -> Duration {
        Duration::from_secs(self.ttl())
    }

    /// Returns the refresh deadline as a [`Duration`] since epoch.
    #[must_use]
    pub fn refresh_deadline_duration(&self) -> Duration {
        Duration::from_secs(self.refresh_deadline())
    }
}

fn unix_time_now() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use norito::{decode_from_bytes, to_bytes};

    use super::*;

    fn sample_advert(now: u64) -> ProviderAdvertV1 {
        let issued_at = now;
        let expires_at = now + REFRESH_RECOMMENDATION_SECS * 2;
        ProviderAdvertV1 {
            version: PROVIDER_ADVERT_VERSION_V1,
            issued_at,
            expires_at,
            body: ProviderAdvertBodyV1 {
                provider_id: [0u8; 32],
                profile_id: "sorafs.sf1@1.0.0".to_owned(),
                profile_aliases: Some(vec!["sorafs.sf1@1.0.0".to_owned(), "sorafs-sf1".to_owned()]),
                stake: StakePointer {
                    pool_id: [1u8; 32],
                    stake_amount: 1_000_000,
                },
                qos: QosHints {
                    availability: AvailabilityTier::Hot,
                    max_retrieval_latency_ms: 1_500,
                    max_concurrent_streams: 32,
                },
                capabilities: vec![
                    CapabilityTlv {
                        cap_type: CapabilityType::ToriiGateway,
                        payload: Vec::new(),
                    },
                    CapabilityTlv {
                        cap_type: CapabilityType::ChunkRangeFetch,
                        payload: ProviderCapabilityRangeV1 {
                            max_chunk_span: 32,
                            min_granularity: 8,
                            supports_sparse_offsets: true,
                            requires_alignment: false,
                            supports_merkle_proof: true,
                        }
                        .to_bytes()
                        .expect("encode range capability"),
                    },
                ],
                endpoints: vec![AdvertEndpoint {
                    kind: EndpointKind::Torii,
                    host_pattern: "storage.example.com".to_owned(),
                    metadata: vec![EndpointMetadata {
                        key: EndpointMetadataKey::Region,
                        value: b"global".to_vec(),
                    }],
                }],
                rendezvous_topics: vec![RendezvousTopic {
                    topic: "sorafs.sf1.primary".to_owned(),
                    region: "global".to_owned(),
                }],
                path_policy: PathDiversityPolicy {
                    min_guard_weight: 10,
                    max_same_asn_per_path: 1,
                    max_same_pool_per_path: 1,
                },
                notes: None,
                stream_budget: Some(StreamBudgetV1 {
                    max_in_flight: 8,
                    max_bytes_per_sec: 10_000_000,
                    burst_bytes: Some(5_000_000),
                }),
                transport_hints: Some(vec![TransportHintV1 {
                    protocol: TransportProtocol::ToriiHttpRange,
                    priority: 0,
                }]),
            },
            signature: AdvertSignature {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: vec![7u8; 32],
                signature: vec![9u8; 64],
            },
            signature_strict: true,
            allow_unknown_capabilities: false,
        }
    }

    #[test]
    fn advert_roundtrip() {
        let advert = sample_advert(1_700_000_000);
        advert.body.validate().unwrap();
        advert.validate_with_body(1_700_000_000).unwrap();

        let bytes = norito::to_bytes(&advert).expect("serialize advert");
        let decoded: ProviderAdvertV1 = norito::decode_from_bytes(&bytes).expect("decode advert");
        assert_eq!(decoded, advert);
    }

    #[test]
    fn ttl_enforced() {
        let mut advert = sample_advert(1_700_000_000);
        advert.expires_at = advert.issued_at + MAX_ADVERT_TTL_SECS + 1;
        let err = advert.validate(1_700_000_000).unwrap_err();
        assert!(matches!(
            err,
            AdvertValidationError::TtlOutOfRange { ttl, max } if ttl == MAX_ADVERT_TTL_SECS + 1 && max == MAX_ADVERT_TTL_SECS
        ));
    }

    #[test]
    fn refresh_deadline_tracks_half_ttl() {
        let mut advert = sample_advert(1_700_000_000);
        advert.expires_at = advert.issued_at + 2 * 60 * 60;
        assert_eq!(advert.refresh_deadline(), advert.issued_at + 60 * 60);

        advert.expires_at = advert.issued_at + 3;
        assert_eq!(advert.refresh_deadline(), advert.issued_at + 2);
    }

    #[test]
    fn detects_missing_endpoints() {
        let mut advert = sample_advert(1_700_000_000);
        advert.body.endpoints.clear();
        let err = advert.validate_with_body(1_700_000_000).unwrap_err();
        assert_eq!(err, AdvertValidationError::MissingEndpoints);
    }

    #[test]
    fn qos_validation() {
        let mut advert = sample_advert(1_700_000_000);
        advert.body.qos.max_concurrent_streams = 0;
        let err = advert.body.validate().unwrap_err();
        assert_eq!(err, AdvertValidationError::InvalidQos);
    }

    #[test]
    fn stream_budget_option_roundtrip() {
        let budget = StreamBudgetV1 {
            max_in_flight: 4,
            max_bytes_per_sec: 2_000_000,
            burst_bytes: Some(1_000_000),
        };
        let bytes = to_bytes(&Some(budget)).expect("encode option");
        let decoded: Option<StreamBudgetV1> = decode_from_bytes(&bytes).expect("decode option");
        assert_eq!(decoded, Some(budget));
    }

    #[test]
    fn builder_constructs_advert() {
        let mut builder = ProviderAdvertV1::builder();
        let _ = builder
            .profile_id("sorafs.sf1@1.0.0")
            .profile_aliases(vec!["sorafs.sf1@1.0.0".to_owned(), "sorafs-sf1".to_owned()])
            .provider_id([0u8; 32])
            .stake_pool_id([1u8; 32])
            .stake_amount(1_000_000)
            .availability(AvailabilityTier::Hot)
            .max_retrieval_latency_ms(1_500)
            .max_concurrent_streams(32)
            .path_policy_min_guard_weight(5)
            .path_policy_max_same_asn_per_path(2)
            .path_policy_max_same_pool_per_path(2)
            .notes("primary-provider");
        let _ = builder.add_capability(CapabilityTlv {
            cap_type: CapabilityType::ToriiGateway,
            payload: Vec::new(),
        });
        let range_capability = ProviderCapabilityRangeV1 {
            max_chunk_span: 24,
            min_granularity: 8,
            supports_sparse_offsets: true,
            requires_alignment: false,
            supports_merkle_proof: true,
        };
        builder
            .add_range_capability(range_capability)
            .expect("encode range capability");
        let _ = builder.stream_budget(StreamBudgetV1 {
            max_in_flight: 4,
            max_bytes_per_sec: 6_000_000,
            burst_bytes: Some(3_000_000),
        });
        let _ = builder.add_transport_hint(TransportHintV1 {
            protocol: TransportProtocol::ToriiHttpRange,
            priority: 0,
        });
        let _ = builder.add_endpoint(AdvertEndpoint {
            kind: EndpointKind::Torii,
            host_pattern: "storage.example.com".into(),
            metadata: vec![EndpointMetadata {
                key: EndpointMetadataKey::Region,
                value: b"global".to_vec(),
            }],
        });
        let _ = builder.add_topic(RendezvousTopic {
            topic: "sorafs.sf1.primary".into(),
            region: "global".into(),
        });
        let _ = builder
            .signature(SignatureAlgorithm::Ed25519, vec![7u8; 32], vec![9u8; 64])
            .ttl_secs(3600)
            .issued_at(1_700_000_000)
            .allow_unknown_capabilities(true);

        let advert = builder.build().expect("builder constructs advert");
        advert
            .validate_with_body(1_700_000_100)
            .expect("advert validates");
        assert_eq!(advert.body.capabilities.len(), 2);
        let range_payload = advert
            .body
            .capabilities
            .iter()
            .find(|cap| cap.cap_type == CapabilityType::ChunkRangeFetch)
            .expect("range capability present");
        assert_eq!(
            ProviderCapabilityRangeV1::from_bytes(&range_payload.payload).unwrap(),
            range_capability
        );
        assert_eq!(advert.body.endpoints.len(), 1);
        assert_eq!(advert.body.rendezvous_topics.len(), 1);
        assert!(advert.allow_unknown_capabilities);
        assert!(advert.body.stream_budget.is_some());
        assert_eq!(
            advert
                .body
                .transport_hints
                .as_ref()
                .map(|hints| hints.len()),
            Some(1)
        );
    }

    #[test]
    fn stream_budget_validation() {
        let err = StreamBudgetV1 {
            max_in_flight: 0,
            max_bytes_per_sec: 1,
            burst_bytes: None,
        }
        .validate()
        .unwrap_err();
        assert_eq!(err, StreamBudgetError::InvalidMaxInFlight);

        let err = StreamBudgetV1 {
            max_in_flight: 1,
            max_bytes_per_sec: 0,
            burst_bytes: None,
        }
        .validate()
        .unwrap_err();
        assert_eq!(err, StreamBudgetError::InvalidMaxBytesPerSec);

        let err = StreamBudgetV1 {
            max_in_flight: 1,
            max_bytes_per_sec: 32,
            burst_bytes: Some(0),
        }
        .validate()
        .unwrap_err();
        assert_eq!(err, StreamBudgetError::InvalidBurstBytes);

        let err = StreamBudgetV1 {
            max_in_flight: 1,
            max_bytes_per_sec: 32,
            burst_bytes: Some(64),
        }
        .validate()
        .unwrap_err();
        assert_eq!(
            err,
            StreamBudgetError::BurstExceedsRate {
                burst: 64,
                rate: 32
            }
        );
    }

    #[test]
    fn transport_hint_priority_validation() {
        let hint = TransportHintV1 {
            protocol: TransportProtocol::ToriiHttpRange,
            priority: 42,
        };
        let err = hint.validate().unwrap_err();
        assert_eq!(err, TransportHintError::InvalidPriority);
    }

    #[test]
    fn duplicate_transport_protocol_rejected() {
        let mut advert = sample_advert(1_700_000_000);
        advert.body.transport_hints = Some(vec![
            TransportHintV1 {
                protocol: TransportProtocol::ToriiHttpRange,
                priority: 0,
            },
            TransportHintV1 {
                protocol: TransportProtocol::ToriiHttpRange,
                priority: 1,
            },
        ]);
        let err = advert.body.validate().unwrap_err();
        assert_eq!(err, AdvertValidationError::DuplicateTransportProtocol);
    }

    #[test]
    fn soranet_transport_requires_capability() {
        let mut advert = sample_advert(1_700_000_000);
        advert.body.transport_hints = Some(vec![TransportHintV1 {
            protocol: TransportProtocol::SoraNetRelay,
            priority: 0,
        }]);
        let err = advert.body.validate().unwrap_err();
        assert_eq!(
            err,
            AdvertValidationError::SoranetTransportWithoutCapability
        );
    }

    #[test]
    fn soranet_transport_accepts_matching_capability() {
        let mut advert = sample_advert(1_700_000_000);
        advert.body.transport_hints = Some(vec![TransportHintV1 {
            protocol: TransportProtocol::SoraNetRelay,
            priority: 0,
        }]);
        let pq_payload = ProviderCapabilitySoranetPqV1 {
            supports_guard: true,
            supports_majority: false,
            supports_strict: false,
        }
        .to_bytes()
        .expect("encode soranet_pq");
        advert.body.capabilities.push(CapabilityTlv {
            cap_type: CapabilityType::SoraNetHybridPq,
            payload: pq_payload,
        });
        advert.body.validate().unwrap();
    }

    #[test]
    fn invalid_range_capability_rejected() {
        let mut advert = sample_advert(1_700_000_000);
        let mut payload = Vec::with_capacity(9);
        payload.extend_from_slice(&8u32.to_le_bytes());
        payload.extend_from_slice(&16u32.to_le_bytes());
        payload.push(0);
        advert.body.capabilities = vec![CapabilityTlv {
            cap_type: CapabilityType::ChunkRangeFetch,
            payload,
        }];
        let err = advert.body.validate().unwrap_err();
        assert!(matches!(
            err,
            AdvertValidationError::InvalidRangeCapability(
                RangeCapabilityError::GranularityExceedsSpan { .. }
            ) | AdvertValidationError::InvalidRangeCapabilityPayload
        ));
    }

    #[test]
    fn soranet_pq_payload_roundtrip() {
        let pq = ProviderCapabilitySoranetPqV1 {
            supports_guard: true,
            supports_majority: true,
            supports_strict: true,
        };
        let bytes = pq.to_bytes().expect("encode soranet_pq");
        let decoded = ProviderCapabilitySoranetPqV1::from_bytes(&bytes).expect("decode soranet_pq");
        assert_eq!(decoded, pq);
    }

    #[test]
    fn soranet_pq_requires_guard_and_majority() {
        let invalid_guard = ProviderCapabilitySoranetPqV1 {
            supports_guard: false,
            supports_majority: true,
            supports_strict: false,
        };
        let err = invalid_guard.to_bytes().unwrap_err();
        assert_eq!(err, PqCapabilityError::MissingGuardSupport);

        let invalid_strict = ProviderCapabilitySoranetPqV1 {
            supports_guard: true,
            supports_majority: false,
            supports_strict: true,
        };
        let err = invalid_strict.to_bytes().unwrap_err();
        assert_eq!(err, PqCapabilityError::StrictWithoutMajority);
    }

    #[test]
    fn stream_budget_without_range_capability_rejected() {
        let mut advert = sample_advert(1_700_000_000);
        advert
            .body
            .capabilities
            .retain(|cap| cap.cap_type != CapabilityType::ChunkRangeFetch);
        let err = advert.body.validate().unwrap_err();
        assert_eq!(err, AdvertValidationError::RangeMetadataWithoutCapability);
    }

    #[test]
    fn transport_hints_without_range_capability_rejected() {
        let mut advert = sample_advert(1_700_000_000);
        advert.body.stream_budget = None;
        advert
            .body
            .capabilities
            .retain(|cap| cap.cap_type != CapabilityType::ChunkRangeFetch);
        let err = advert.body.validate().unwrap_err();
        assert_eq!(err, AdvertValidationError::RangeMetadataWithoutCapability);
    }

    #[test]
    fn empty_transport_hints_rejected() {
        let mut advert = sample_advert(1_700_000_000);
        advert.body.transport_hints = Some(vec![]);
        let err = advert.body.validate().unwrap_err();
        assert_eq!(err, AdvertValidationError::EmptyTransportHints);
    }
}
