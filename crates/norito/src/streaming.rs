//! Norito Streaming Codec (NSC) manifest, control, and telemetry data structures.
//!
//! These types follow the specification in `norito_streaming.md` and provide the
//! serialization surface referenced by the roadmap implementation plan. They do
//! not perform any validation or crypto; callers are responsible for enforcing
//! protocol rules before encoding or after decoding values.
/// `true` when CABAC code paths were compiled (`ENABLE_CABAC=1` set at build time).
pub const CABAC_BUILD_AVAILABLE: bool = cfg!(norito_enable_cabac);
/// `true` when trellis scan code paths were compiled (`ENABLE_TRELLIS=1` set at build time).
pub const TRELLIS_BUILD_AVAILABLE: bool = cfg!(norito_enable_trellis);
/// `true` when bundled rANS code paths were compiled (`ENABLE_RANS_BUNDLES=1` set at build time).
pub const BUNDLED_RANS_BUILD_AVAILABLE: bool = cfg!(norito_enable_rans_bundles);
/// `true` when GPU bundle-acceleration hooks are compiled (requires bundled rANS plus a GPU feature flag).
pub const BUNDLED_RANS_GPU_BUILD_AVAILABLE: bool = cfg!(all(
    norito_enable_rans_bundles,
    any(
        feature = "codec-gpu",
        feature = "codec-gpu-metal",
        feature = "codec-gpu-cuda"
    )
));

use core::{fmt, str::FromStr};

use thiserror::Error;

use crate::{
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
    core::{self as norito_core, DecodeFromSlice, Error as CoreError},
    json,
    json::Value as NoritoJsonValue,
};
/// Blake3-based 32-byte hash used across NSC metadata (chunk commitments, IDs, etc.).
pub type Hash = [u8; 32];
/// Ed25519 signature bytes as specified for manifests and control frames.
pub type Signature = [u8; 64];
/// Timestamp field used by manifests. The spec leaves the exact unit to deployments; NSC uses unix time.
pub type Timestamp = u64;
/// Convenience alias for opaque byte payloads (tickets, encrypted keys, QUIC frames).
pub type Bytes = Vec<u8>;
/// Multi-address string identifying QUIC ingress/egress endpoints.
pub type Multiaddr = String;
/// Canonical account identifier used by the Nexus streaming contracts.
pub type AccountId = String;
/// Data Space identifier (DSID) used to scope tickets and manifests.
pub type DataSpaceId = u64;
/// Identifier referencing a registered zero-knowledge verifier.
pub type VerifierId = [u8; 32];
/// Contract signature emitted by on-chain streaming access contracts.
pub type ContractSignature = Signature;
/// ISO-style region code used in ticket policies (e.g., "us", "eu").
pub type RegionCode = String;

pub use codec::{
    BundleContextRemap, BundleContextStats, BundledStats, BundledTelemetry, BundledToken,
    load_bundle_context_remap_from_json,
};

#[inline]
fn saturating_usize_to_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[inline]
fn saturating_usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

/// Video profile identifier (`baseline`, `uhd_main`, `uhd_ai`, etc.).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, NoritoSerialize, NoritoDeserialize,
)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct ProfileId(pub u16);

impl ProfileId {
    /// Baseline SDR profile (`ProfileId = 0`).
    pub const BASELINE: Self = Self(0);
    /// HDR main profile (`ProfileId = 1`).
    pub const UHD_MAIN: Self = Self(1);
    /// HDR + neural residual profile (`ProfileId = 2`).
    pub const UHD_AI: Self = Self(2);
}

/// Entropy coder advertised by manifests and segment headers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, Default)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum EntropyMode {
    /// Multi-bit bundled rANS encoder.
    #[default]
    RansBundled,
}

impl EntropyMode {
    #[must_use]
    pub const fn is_bundled(self) -> bool {
        true
    }

    #[must_use]
    fn as_str(self) -> &'static str {
        "rans_bundled"
    }
}

impl fmt::Display for EntropyMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Rate-distortion optimizer mode for the bundled entropy path.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum RdoMode {
    /// Skip RDO and emit coefficients as-is.
    #[default]
    None,
    /// Trellis-free DP optimizer with explicit energy buckets.
    DynamicProgramming,
    /// DP optimizer guided by a small int8 neural predictor.
    Neural,
    /// Perceptual lambda schedule tuned for SSIM-like behaviour.
    Perceptual,
}

impl RdoMode {
    pub const fn is_enabled(self) -> bool {
        !matches!(self, Self::None)
    }
}

impl FromStr for EntropyMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "rans_bundled" | "rans-bundled" => Ok(Self::RansBundled),
            _ => Err(()),
        }
    }
}

/// Bitfield describing manifest feature flags (HDR, neural bundle, privacy overlay, etc.).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct CapabilityFlags(u32);

impl CapabilityFlags {
    /// Feature bit enabling feedback hint frames (`ManifestV1::feedback_hint`).
    pub const FEATURE_FEEDBACK_HINTS: u32 = 1 << 0;
    /// Feature bit marking the publisher as a privacy overlay provider.
    pub const FEATURE_PRIVACY_PROVIDER: u32 = 1 << 1;
    /// Feature bit advertising SM transaction/signature support across the streaming control plane.
    pub const FEATURE_SM_TRANSACTIONS: u32 = 1 << 8;
    /// Feature bit advertising bundled rANS entropy support.
    pub const FEATURE_ENTROPY_BUNDLED: u32 = 1 << 9;
    /// Feature bit indicating bundled rANS paths execute on CPU SIMD accelerators.
    pub const FEATURE_BUNDLE_ACCEL_CPU_SIMD: u32 = 1 << 13;
    /// Feature bit indicating bundled rANS paths execute on GPU accelerators.
    pub const FEATURE_BUNDLE_ACCEL_GPU: u32 = 1 << 14;

    /// Creates a new flag wrapper from raw bits.
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns the underlying bit representation.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Checks whether all bits in `mask` are set.
    pub const fn contains(self, mask: u32) -> bool {
        (self.0 & mask) == mask
    }

    /// Returns a new flag set with the provided mask applied.
    pub const fn insert(self, mask: u32) -> Self {
        Self(self.0 | mask)
    }

    /// Returns a new flag set with the provided mask removed.
    pub const fn remove(self, mask: u32) -> Self {
        Self(self.0 & !mask)
    }
}

/// Acceleration backend used for bundled entropy pipelines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum BundleAcceleration {
    /// No dedicated acceleration.
    #[default]
    None,
    /// CPU SIMD accelerated bundle processing.
    CpuSimd,
    /// GPU accelerated bundle processing.
    Gpu,
}

impl BundleAcceleration {
    /// Capability bit mask representing this acceleration choice.
    pub const fn capability_mask(self) -> u32 {
        match self {
            Self::None => 0,
            Self::CpuSimd => CapabilityFlags::FEATURE_BUNDLE_ACCEL_CPU_SIMD,
            Self::Gpu => CapabilityFlags::FEATURE_BUNDLE_ACCEL_GPU,
        }
    }
}

/// Privacy relay capabilities advertised in `PrivacyRoute`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct PrivacyCapabilities(u32);

impl PrivacyCapabilities {
    /// Creates a new capability wrapper from raw bits.
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns the raw bitset.
    pub const fn bits(self) -> u32 {
        self.0
    }
}

/// FEC configuration advertised at the manifest or feedback layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct FecParameters {
    /// Active scheme identifier.
    pub scheme: FecScheme,
    /// Optional sliding-window step (segments/chunks) when applicable.
    pub window_step: Option<u8>,
    /// Number of parity symbols emitted for each protected window.
    pub parity_symbols: Option<u8>,
}

/// Per-layer rate/FEC/storage hints emitted by the adaptive controller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct LayerFeedback {
    /// Layer index (temporal/spatial combined bit index as defined in the spec).
    pub layer_id: u8,
    /// Lower bitrate bound in kbps for the target layer.
    pub min_target_kbps: u32,
    /// Upper bitrate bound in kbps for the target layer.
    pub max_target_kbps: u32,
    /// Optional override for the preferred storage class for this layer.
    pub storage_hint: Option<StorageClass>,
}

/// Feedback emitted per segment so viewers and relays can adapt.
#[derive(Clone, Debug, PartialEq, Eq, Default, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct FeedbackHint {
    /// Bitrate hints per layer.
    pub layer_hints: Vec<LayerFeedback>,
    /// Optional cadence override for `ReceiverReport` frames (milliseconds).
    pub report_interval_ms: Option<u16>,
    /// Active FEC parameters.
    pub fec: Option<FecParameters>,
}

/// HPKE suites supported by the transport negotiation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum HpkeSuite {
    /// Kyber768 + ChaCha20Poly1305 operated in AuthPsk mode (`suite_id = 0x0001`).
    Kyber768AuthPsk,
    /// Kyber1024 + ChaCha20Poly1305 operated in AuthPsk mode (`suite_id = 0x0002`).
    Kyber1024AuthPsk,
}

impl HpkeSuite {
    /// Numeric identifier used by transport capability hashing.
    #[must_use]
    pub const fn suite_id(self) -> u16 {
        match self {
            Self::Kyber768AuthPsk => 0x0001,
            Self::Kyber1024AuthPsk => 0x0002,
        }
    }

    /// Converts the suite into a bit index used by [`HpkeSuiteMask`].
    #[must_use]
    pub const fn bit(self) -> u16 {
        match self {
            Self::Kyber768AuthPsk => 0,
            Self::Kyber1024AuthPsk => 1,
        }
    }
}

/// Bitmask describing the HPKE suites supported by an endpoint.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct HpkeSuiteMask(u16);

impl HpkeSuiteMask {
    /// Empty mask (no suites supported).
    pub const EMPTY: Self = Self(0);
    /// Convenience constant containing Suite #1 (Kyber768).
    pub const KYBER768: Self = Self(1 << HpkeSuite::Kyber768AuthPsk.bit());
    /// Convenience constant containing Suite #2 (Kyber1024).
    pub const KYBER1024: Self = Self(1 << HpkeSuite::Kyber1024AuthPsk.bit());

    /// Construct a mask that advertises a single suite.
    #[must_use]
    pub const fn from_suite(suite: HpkeSuite) -> Self {
        Self(1 << suite.bit())
    }

    /// Returns the raw bit representation.
    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Construct a mask from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    /// Returns true when the given suite is advertised in this mask.
    #[must_use]
    pub const fn contains(self, suite: HpkeSuite) -> bool {
        (self.0 & (1 << suite.bit())) != 0
    }

    /// Computes the intersection of two masks.
    #[must_use]
    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    /// Returns the lowest-index suite present in the mask.
    #[must_use]
    pub const fn lowest(self) -> Option<HpkeSuite> {
        if self.contains(HpkeSuite::Kyber768AuthPsk) {
            Some(HpkeSuite::Kyber768AuthPsk)
        } else if self.contains(HpkeSuite::Kyber1024AuthPsk) {
            Some(HpkeSuite::Kyber1024AuthPsk)
        } else {
            None
        }
    }
}

/// Privacy bucket configuration used when redacting telemetry counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum PrivacyBucketGranularity {
    /// Standard NSC v1 aggregation rules (1% loss buckets, 1 ms latency bands).
    StandardV1,
}

/// Transport capability advertisement exchanged during the QUIC handshake.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct TransportCapabilities {
    /// HPKE suite mask describing supported cipher suites.
    pub hpke_suites: HpkeSuiteMask,
    /// Whether the endpoint can send/receive QUIC DATAGRAM frames.
    pub supports_datagram: bool,
    /// Maximum DATAGRAM payload size (bytes) the endpoint can accept per segment chunk.
    pub max_segment_datagram_size: u16,
    /// Preferred interval (milliseconds) between `FeedbackHint` frames.
    pub fec_feedback_interval_ms: u16,
    /// Privacy bucket configuration applied to telemetry exports.
    pub privacy_bucket_granularity: PrivacyBucketGranularity,
}

impl TransportCapabilities {
    /// Convenience helper building the mandatory Kyber768 + DATAGRAM profile.
    #[must_use]
    pub const fn kyber768_default() -> Self {
        Self {
            hpke_suites: HpkeSuiteMask::KYBER768,
            supports_datagram: true,
            max_segment_datagram_size: u16::MAX,
            fec_feedback_interval_ms: 250,
            privacy_bucket_granularity: PrivacyBucketGranularity::StandardV1,
        }
    }
}

/// Resolved transport configuration derived from the two advertised capability sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportCapabilityResolution {
    /// Selected HPKE suite.
    pub hpke_suite: HpkeSuite,
    /// Indicates whether DATAGRAM delivery is enabled for this session.
    pub use_datagram: bool,
    /// Maximum DATAGRAM payload size agreed upon by the peers.
    pub max_segment_datagram_size: u16,
    /// Interval (milliseconds) between feedback hints.
    pub fec_feedback_interval_ms: u16,
    /// Privacy bucket configuration negotiated for telemetry exports.
    pub privacy_bucket_granularity: PrivacyBucketGranularity,
}

impl TransportCapabilityResolution {
    /// Compute the canonical hash recorded inside manifests for auditing.
    #[must_use]
    pub fn capabilities_hash(&self) -> Hash {
        const DOMAIN: &[u8] = b"nsc-transport-capabilities";
        let mut hasher = blake3::Hasher::new();
        hasher.update(DOMAIN);
        hasher.update(&self.hpke_suite.suite_id().to_le_bytes());
        hasher.update(&[self.use_datagram as u8]);
        hasher.update(&self.max_segment_datagram_size.to_le_bytes());
        hasher.update(&self.fec_feedback_interval_ms.to_le_bytes());
        hasher.update(&[self.privacy_bucket_granularity as u8]);
        hasher.finalize().into()
    }
}

/// Errors encountered when resolving advertised transport capabilities.
#[derive(Clone, Copy, Debug, thiserror::Error, PartialEq, Eq)]
pub enum TransportCapabilityError {
    /// The two endpoints have no compatible HPKE suites.
    #[error("no shared HPKE suite between peers")]
    NoSharedHpkeSuite,
    /// Privacy bucket configurations could not be reconciled.
    #[error("privacy bucket granularity mismatch: local {local:?}, remote {remote:?}")]
    PrivacyBucketMismatch {
        /// Local privacy bucket configuration.
        local: PrivacyBucketGranularity,
        /// Remote privacy bucket configuration.
        remote: PrivacyBucketGranularity,
    },
    /// The advertised DATAGRAM size was zero while the peer expected DATAGRAM support.
    #[error("invalid datagram size advertised ({0} bytes)")]
    InvalidDatagramSize(u16),
}

/// Resolve the transport capabilities advertised by two endpoints.
pub fn resolve_transport_capabilities(
    local: &TransportCapabilities,
    remote: &TransportCapabilities,
) -> Result<TransportCapabilityResolution, TransportCapabilityError> {
    let shared = local.hpke_suites.intersection(remote.hpke_suites);
    let hpke_suite = shared
        .lowest()
        .ok_or(TransportCapabilityError::NoSharedHpkeSuite)?;

    if local.privacy_bucket_granularity != remote.privacy_bucket_granularity {
        return Err(TransportCapabilityError::PrivacyBucketMismatch {
            local: local.privacy_bucket_granularity,
            remote: remote.privacy_bucket_granularity,
        });
    }

    let use_datagram = local.supports_datagram && remote.supports_datagram;
    let max_segment_datagram_size = if use_datagram {
        let negotiated = local
            .max_segment_datagram_size
            .min(remote.max_segment_datagram_size);
        if negotiated == 0 {
            return Err(TransportCapabilityError::InvalidDatagramSize(negotiated));
        }
        negotiated
    } else {
        0
    };

    let fec_feedback_interval_ms = local
        .fec_feedback_interval_ms
        .max(remote.fec_feedback_interval_ms);

    Ok(TransportCapabilityResolution {
        hpke_suite,
        use_datagram,
        max_segment_datagram_size,
        fec_feedback_interval_ms,
        privacy_bucket_granularity: local.privacy_bucket_granularity,
    })
}

/// Storage tier for a segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum StorageClass {
    /// Short-retention, lower-cost storage.
    Ephemeral,
    /// Long-retention, higher-cost storage.
    Permanent,
}

/// Available FEC schemes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum FecScheme {
    /// Mandatory RS 12/10 configuration.
    Rs12_10,
    /// Sliding-window RS 14/10 variant.
    RsWin14_10,
    /// Sliding-window RS 18/14 variant.
    Rs18_14,
}

/// Encryption suite negotiated for segments/control frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum EncryptionSuite {
    /// X25519 + ChaCha20-Poly1305 using the referenced key fingerprint.
    X25519ChaCha20Poly1305(Hash),
    /// Kyber768 + XChaCha20-Poly1305 using the referenced key fingerprint.
    Kyber768XChaCha20Poly1305(Hash),
}

/// Basic manifest metadata visible to viewers.
#[derive(Clone, Debug, PartialEq, Eq, Default, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct StreamMetadata {
    pub title: String,
    pub description: Option<String>,
    pub access_policy_id: Option<Hash>,
    pub tags: Vec<String>,
}

/// Privacy relay descriptor used in manifest routes.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct PrivacyRelay {
    pub relay_id: Hash,
    pub endpoint: Multiaddr,
    pub key_fingerprint: Hash,
    pub capabilities: PrivacyCapabilities,
}

/// Authentication posture applied when exiting a SoraNet circuit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum SoranetAccessKind {
    /// Exit relays only expose read-only content (no authenticated mutations).
    ReadOnly,
    /// Exit relays require viewer authentication/tickets before forwarding.
    Authenticated,
}

/// Stream tags advertised by SoraNet relays to differentiate exit adapters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum SoranetStreamTag {
    /// Norito RPC/streaming bridge.
    #[default]
    NoritoStream,
    /// Kaigi real-time conferencing bridge.
    Kaigi,
}

/// Blinded identifier for the streaming circuit advertised by a relay directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct SoranetChannelId(pub [u8; 32]);

impl SoranetChannelId {
    /// Total number of bytes carried by the identifier.
    pub const LENGTH: usize = 32;

    /// Construct a blinded channel identifier from raw bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Borrow the underlying byte array.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<[u8; 32]> for SoranetChannelId {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl From<SoranetChannelId> for [u8; 32] {
    fn from(id: SoranetChannelId) -> Self {
        id.0
    }
}

impl AsRef<[u8; 32]> for SoranetChannelId {
    fn as_ref(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Parameters required to bridge a privacy route over a SoraNet circuit.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct SoranetRoute {
    /// Blinded channel identifier negotiated during circuit admission.
    pub channel_id: SoranetChannelId,
    /// Exit relay multiaddr that forwards decrypted traffic to Torii.
    pub exit_multiaddr: Multiaddr,
    /// Optional padding budget (milliseconds) applied for low-latency tuning.
    pub padding_budget_ms: Option<u16>,
    /// Access posture enforced by the exit relay.
    pub access_kind: SoranetAccessKind,
    /// Stream tag describing the exit adapter.
    #[norito(default)]
    pub stream_tag: SoranetStreamTag,
}

impl<'a> DecodeFromSlice<'a> for SoranetRoute {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), CoreError> {
        norito_core::decode_field_canonical::<Self>(bytes)
    }
}

/// An entry/exit route authorizing privacy-preserving transport.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct PrivacyRoute {
    pub route_id: Hash,
    pub entry: PrivacyRelay,
    pub exit: PrivacyRelay,
    pub ticket_entry: Bytes,
    pub ticket_exit: Bytes,
    pub expiry_segment: u64,
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub soranet: Option<SoranetRoute>,
}

/// Optional neural enhancement bundle metadata.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct NeuralBundle {
    pub bundle_id: String,
    pub weights_sha256: Hash,
    pub activation_scale: Vec<i16>,
    pub bias: Vec<i32>,
    pub metadata_signature: Signature,
    pub metal_shader_sha256: Option<Hash>,
    pub cuda_ptx_sha256: Option<Hash>,
}

/// Bitfield describing which playback profiles a capability ticket authorizes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct TicketCapabilities(u32);

impl TicketCapabilities {
    /// Capability flag allowing live streaming access.
    pub const LIVE: u32 = 1 << 0;
    /// Capability flag allowing video-on-demand playback.
    pub const VOD: u32 = 1 << 1;
    /// Capability flag unlocking premium rendering profiles (e.g., UHD AI).
    pub const PREMIUM_PROFILE: u32 = 1 << 2;
    /// Capability flag enabling HDR ladder selection.
    pub const HDR: u32 = 1 << 3;
    /// Capability flag enabling spatial audio playback.
    pub const SPATIAL_AUDIO: u32 = 1 << 4;

    /// Construct a capability flag wrapper from raw bits.
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Retrieve the underlying bit representation.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Check whether all flags in `mask` are enabled.
    pub const fn contains(self, mask: u32) -> bool {
        (self.0 & mask) == mask
    }

    /// Return a new flag set with the provided `mask` inserted.
    pub const fn insert(self, mask: u32) -> Self {
        Self(self.0 | mask)
    }

    /// Return a new flag set with the provided `mask` cleared.
    pub const fn remove(self, mask: u32) -> Self {
        Self(self.0 & !mask)
    }
}

/// Optional policy constraints embedded in capability tickets.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct TicketPolicy {
    /// Maximum number of relays that may concurrently serve this ticket.
    pub max_relays: u16,
    /// Regions authorized to serve the ticket (ISO-style codes).
    pub allowed_regions: Vec<RegionCode>,
    /// Optional bandwidth cap in kilobits per second.
    pub max_bandwidth_kbps: Option<u32>,
}

/// Streaming capability ticket embedding ZK commitments and policy metadata.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct StreamingTicket {
    pub ticket_id: Hash,
    pub owner: AccountId,
    pub dsid: DataSpaceId,
    pub lane_id: u8,
    pub settlement_bucket: u64,
    pub start_slot: u64,
    pub expire_slot: u64,
    pub prepaid_teu: u128,
    pub chunk_teu: u32,
    pub fanout_quota: u16,
    pub key_commitment: Hash,
    pub nonce: u64,
    pub contract_sig: ContractSignature,
    pub commitment: Hash,
    pub nullifier: Hash,
    pub proof_id: VerifierId,
    pub issued_at: Timestamp,
    pub expires_at: Timestamp,
    pub policy: Option<TicketPolicy>,
    pub capabilities: TicketCapabilities,
}

/// Ticket revocation payload carrying nullifier metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct TicketRevocation {
    pub ticket_id: Hash,
    pub nullifier: Hash,
    pub reason_code: u16,
    pub revocation_signature: ContractSignature,
}

/// Manifest describing a single NSC segment.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct ManifestV1 {
    pub stream_id: Hash,
    pub protocol_version: u16,
    pub segment_number: u64,
    pub published_at: Timestamp,
    pub profile: ProfileId,
    #[norito(default)]
    pub entropy_mode: EntropyMode,
    #[norito(default)]
    pub entropy_tables_checksum: Option<Hash>,
    pub da_endpoint: Multiaddr,
    pub chunk_root: Hash,
    pub content_key_id: u64,
    pub nonce_salt: Hash,
    pub chunk_descriptors: Vec<ChunkDescriptor>,
    pub transport_capabilities_hash: Hash,
    pub encryption_suite: EncryptionSuite,
    pub fec_suite: FecScheme,
    pub privacy_routes: Vec<PrivacyRoute>,
    pub neural_bundle: Option<NeuralBundle>,
    pub audio_summary: Option<AudioTrackSummary>,
    pub public_metadata: StreamMetadata,
    pub capabilities: CapabilityFlags,
    pub signature: Signature,
}

/// Segment header persisted alongside chunk data.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct SegmentHeader {
    pub segment_number: u64,
    pub profile: ProfileId,
    #[norito(default)]
    pub entropy_mode: EntropyMode,
    #[norito(default)]
    pub entropy_tables_checksum: Option<Hash>,
    pub encryption_suite: EncryptionSuite,
    pub layer_bitmap: u32,
    pub chunk_merkle_root: Hash,
    pub chunk_count: u16,
    pub timeline_start_ns: u64,
    pub duration_ns: u32,
    pub feedback_hint: FeedbackHint,
    pub content_key_id: u64,
    pub nonce_salt: Hash,
    pub storage_class: StorageClass,
    pub audio_summary: Option<AudioTrackSummary>,
    #[norito(default)]
    pub bundle_acceleration: BundleAcceleration,
}

/// Chunk descriptor within a segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct ChunkDescriptor {
    pub chunk_id: u16,
    pub offset: u32,
    pub length: u32,
    pub commitment: Hash,
    pub parity: bool,
}

/// Merkle proof for DA validation.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct MerkleProof {
    pub chunk_id: u16,
    pub sibling_hashes: Vec<Hash>,
    pub directions: Vec<bool>,
}

/// Data availability proof submitted on-chain.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct DataAvailabilityProof {
    pub segment_number: u64,
    pub chunk_root: Hash,
    pub content_key_id: u64,
    pub chunk_ids: Vec<u16>,
    pub merkle_proofs: Vec<MerkleProof>,
    pub storage_commitment: Hash,
    pub validator_signature: Signature,
}

/// Control stream frames exchanged over QUIC.
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum ControlFrame {
    ManifestAnnounce(Box<ManifestAnnounceFrame>),
    ChunkRequest(ChunkRequestFrame),
    ChunkAcknowledge(ChunkAcknowledgeFrame),
    TransportCapabilities(TransportCapabilitiesFrame),
    CapabilityReport(CapabilityReport),
    CapabilityAck(CapabilityAck),
    FeedbackHint(FeedbackHintFrame),
    ReceiverReport(ReceiverReport),
    KeyUpdate(KeyUpdate),
    ContentKeyUpdate(ContentKeyUpdate),
    PrivacyRouteUpdate(PrivacyRouteUpdate),
    PrivacyRouteAck(PrivacyRouteAckFrame),
    Error(ControlErrorFrame),
}

/// Error codes used in control frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum ErrorCode {
    UnknownChunk,
    AccessDenied,
    RateLimited,
    ProtocolViolation,
}

/// Session key update frame payload.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct KeyUpdate {
    pub session_id: Hash,
    pub suite: EncryptionSuite,
    pub protocol_version: u16,
    pub pub_ephemeral: Bytes,
    pub key_counter: u64,
    pub signature: Signature,
}

/// Group content key update payload.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct ContentKeyUpdate {
    pub content_key_id: u64,
    pub gck_wrapped: Bytes,
    pub valid_from_segment: u64,
}

/// Privacy route provisioning payload.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct PrivacyRouteUpdate {
    pub route_id: Hash,
    pub stream_id: Hash,
    pub content_key_id: u64,
    pub valid_from_segment: u64,
    pub valid_until_segment: u64,
    pub exit_token: Bytes,
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub soranet: Option<SoranetRoute>,
}

/// Capability negotiation report.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct CapabilityReport {
    pub stream_id: Hash,
    pub endpoint_role: CapabilityRole,
    pub protocol_version: u16,
    pub max_resolution: Resolution,
    pub hdr_supported: bool,
    pub capture_hdr: bool,
    pub neural_bundles: Vec<String>,
    pub audio_caps: AudioCapability,
    pub feature_bits: CapabilityFlags,
    pub max_datagram_size: u16,
    pub dplpmtud: bool,
}

/// Capability negotiation acknowledgement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct CapabilityAck {
    pub stream_id: Hash,
    pub accepted_version: u16,
    pub negotiated_features: CapabilityFlags,
    pub max_datagram_size: u16,
    pub dplpmtud: bool,
}

/// Endpoint role during capability negotiation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum CapabilityRole {
    Publisher,
    Viewer,
}

/// Audio capability advertisement.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioCapability {
    pub sample_rates: Vec<u32>,
    pub ambisonics: bool,
    pub max_channels: u8,
}

/// Supported output resolutions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum Resolution {
    R720p,
    R1080p,
    R1440p,
    R2160p,
    Custom(ResolutionCustom),
}

/// Audio frame layout (mono, stereo, ambisonics).
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum AudioLayout {
    Mono,
    Stereo,
    FirstOrderAmbisonics,
}

impl AudioLayout {
    #[must_use]
    pub const fn channel_count(self) -> usize {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
            Self::FirstOrderAmbisonics => 4,
        }
    }
}

impl From<AudioLayout> for iroha_audio::ChannelLayout {
    fn from(value: AudioLayout) -> Self {
        match value {
            AudioLayout::Mono => iroha_audio::ChannelLayout::Mono,
            AudioLayout::Stereo => iroha_audio::ChannelLayout::Stereo,
            AudioLayout::FirstOrderAmbisonics => iroha_audio::ChannelLayout::FirstOrderAmbisonics,
        }
    }
}

impl From<iroha_audio::ChannelLayout> for AudioLayout {
    fn from(value: iroha_audio::ChannelLayout) -> Self {
        match value {
            iroha_audio::ChannelLayout::Mono => AudioLayout::Mono,
            iroha_audio::ChannelLayout::Stereo => AudioLayout::Stereo,
            iroha_audio::ChannelLayout::FirstOrderAmbisonics => AudioLayout::FirstOrderAmbisonics,
        }
    }
}

/// Encoded audio frame aligned with a video segment.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioFrame {
    pub sequence: u64,
    pub timestamp_ns: u64,
    pub fec_level: u8,
    pub channel_layout: AudioLayout,
    pub payload: Bytes,
}

/// Summary describing the audio track present in a segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioTrackSummary {
    pub sample_rate: u32,
    pub frame_samples: u16,
    pub frame_duration_ns: u32,
    pub frames_per_segment: u16,
    pub layout: AudioLayout,
    pub fec_level: u8,
}

/// Encoded audio payload accompanying a segment.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct SegmentAudio {
    pub summary: AudioTrackSummary,
    pub frames: Vec<AudioFrame>,
}

impl<'a> crate::core::DecodeFromSlice<'a> for SegmentAudio {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), crate::Error> {
        crate::core::decode_field_canonical::<SegmentAudio>(bytes)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioPcmLengthMismatchInfo {
    pub expected: u64,
    pub found: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioChannelCountMismatchInfo {
    pub expected: u8,
    pub found: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioEncoderSampleCountMismatchInfo {
    pub expected: u16,
    pub found: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioBackendFailureInfo {
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct AudioCodecLayoutMismatchInfo {
    pub expected: AudioLayout,
    pub found: AudioLayout,
}

/// Errors emitted by the audio encoding/decoding layer.
#[derive(Debug, Error)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum AudioCodecError {
    #[error(
        "expected {expected} PCM samples, got {found}",
        expected = .0.expected,
        found = .0.found
    )]
    InvalidPcmLength(AudioPcmLengthMismatchInfo),
    #[error(
        "audio layout mismatch: expected {expected:?}, found {found:?}",
        expected = .0.expected,
        found = .0.found
    )]
    LayoutMismatch(AudioCodecLayoutMismatchInfo),
    #[error("unsupported audio payload version {0}")]
    UnsupportedVersion(u8),
    #[error("audio packet too short")]
    PacketTooShort,
    #[error(
        "invalid channel count: expected {expected}, found {found}",
        expected = .0.expected,
        found = .0.found
    )]
    InvalidChannelCount(AudioChannelCountMismatchInfo),
    #[error(
        "invalid frame sample count: expected {expected}, found {found}",
        expected = .0.expected,
        found = .0.found
    )]
    InvalidSampleCount(AudioEncoderSampleCountMismatchInfo),
    #[error("audio backend unavailable for layout {0:?}")]
    BackendUnavailable(AudioLayout),
    #[error("audio backend failure: {message}", message = .0.message)]
    BackendFailure(AudioBackendFailureInfo),
    #[error("audio backend does not support layout {0:?}")]
    BackendUnsupportedLayout(AudioLayout),
}

impl AudioCodecError {
    fn from_backend(err: iroha_audio::CodecError) -> Self {
        match err {
            iroha_audio::CodecError::InvalidPcmLength { expected, found } => {
                Self::InvalidPcmLength(AudioPcmLengthMismatchInfo {
                    expected: saturating_usize_to_u64(expected),
                    found: saturating_usize_to_u64(found),
                })
            }
            iroha_audio::CodecError::UnsupportedVersion(version) => {
                Self::UnsupportedVersion(version)
            }
            iroha_audio::CodecError::PacketTooShort => Self::PacketTooShort,
            iroha_audio::CodecError::InvalidChannelCount { expected, found } => {
                let expected = expected.unwrap_or(found);
                Self::InvalidChannelCount(AudioChannelCountMismatchInfo { expected, found })
            }
            iroha_audio::CodecError::InvalidSampleCount { expected, found } => {
                Self::InvalidSampleCount(AudioEncoderSampleCountMismatchInfo { expected, found })
            }
            iroha_audio::CodecError::BackendUnavailable { layout } => {
                Self::BackendUnavailable(layout.into())
            }
            iroha_audio::CodecError::LibopusError { message } => {
                Self::BackendFailure(AudioBackendFailureInfo {
                    message: message.to_string(),
                })
            }
            iroha_audio::CodecError::UnsupportedLayout { layout } => {
                Self::BackendUnsupportedLayout(layout.into())
            }
        }
    }
}

impl SegmentAudio {
    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

/// Viewer-side congestion feedback emitted at the negotiated cadence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct FeedbackHintFrame {
    /// Stream identifier associated with the feedback sample.
    pub stream_id: Hash,
    /// Exponential weighted moving average of loss (Q16.16 fixed-point).
    pub loss_ewma_q16: u32,
    /// Latency gradient (Q16.16, signed) over the last interval.
    pub latency_gradient_q16: i32,
    /// Observed round-trip time in milliseconds.
    pub observed_rtt_ms: u16,
    /// Preferred interval (milliseconds) between `ReceiverReport` frames.
    pub report_interval_ms: u16,
    /// Publisher-selected parity budget for the next window.
    pub parity_chunks: u8,
}

/// Receiver telemetry sent during playback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct ReceiverReport {
    pub stream_id: Hash,
    pub latest_segment: u64,
    pub layer_mask: u32,
    pub measured_throughput_kbps: u32,
    pub rtt_ms: u16,
    pub loss_percent_x100: u16,
    pub decoder_buffer_ms: u16,
    pub active_resolution: Resolution,
    pub hdr_active: bool,
    pub ecn_ce_count: u32,
    pub jitter_ms: u16,
    /// Highest delivered datagram/sequence number in the last window.
    pub delivered_sequence: u64,
    /// Redundancy parity chunks applied by the viewer during the last window.
    pub parity_applied: u8,
    /// Recommended parity budget for the next window.
    pub fec_budget: u8,
    /// Optional viewer-provided sync diagnostics for validator enforcement.
    pub sync_diagnostics: Option<SyncDiagnostics>,
}

/// Result of an operational audit routed through the telemetry bridge.
#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct TelemetryAuditOutcome {
    /// Identifier of the audit trace (e.g., `TRACE-TELEMETRY-BRIDGE`).
    pub trace_id: String,
    /// Slot height associated with the audit evaluation.
    pub slot_height: u64,
    /// Reviewer responsible for signing off the audit.
    pub reviewer: String,
    /// Audit verdict (`pass`, `fail`, `mitigated`, etc.).
    pub status: String,
    /// Optional mitigation URL with runbooks or RCA notes.
    pub mitigation_url: Option<String>,
}

/// Telemetry emitted by publisher/decoder implementations.
#[derive(Clone, Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum TelemetryEvent {
    Encode(TelemetryEncodeStats),
    Decode(TelemetryDecodeStats),
    Network(TelemetryNetworkStats),
    Security(TelemetrySecurityStats),
    Energy(TelemetryEnergyStats),
    AuditOutcome(TelemetryAuditOutcome),
}

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct ManifestAnnounceFrame {
    pub manifest: ManifestV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct ChunkRequestFrame {
    pub segment: u64,
    pub chunk_id: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct ChunkAcknowledgeFrame {
    pub segment: u64,
    pub chunk_id: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct TransportCapabilitiesFrame {
    pub endpoint_role: CapabilityRole,
    pub capabilities: TransportCapabilities,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct PrivacyRouteAckFrame {
    pub route_id: Hash,
}

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct ControlErrorFrame {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct ResolutionCustom {
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct TelemetryEncodeStats {
    pub segment: u64,
    pub avg_latency_ms: u16,
    pub dropped_layers: u32,
    pub avg_audio_jitter_ms: u16,
    pub max_audio_jitter_ms: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct TelemetryDecodeStats {
    pub segment: u64,
    pub buffer_ms: u16,
    pub dropped_frames: u16,
    pub max_decode_queue_ms: u16,
    pub avg_av_drift_ms: i16,
    pub max_av_drift_ms: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct TelemetryNetworkStats {
    pub rtt_ms: u16,
    pub loss_percent_x100: u16,
    pub fec_repairs: u32,
    pub fec_failures: u32,
    pub datagram_reinjects: u32,
}

/// Aggregated viewer sync diagnostics transmitted with [`ReceiverReport`] frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct SyncDiagnostics {
    /// Rolling aggregation window covering the metrics, in milliseconds.
    pub window_ms: u16,
    /// Number of samples collected during the window.
    pub samples: u16,
    /// Average audio jitter recorded over the window (milliseconds).
    pub avg_audio_jitter_ms: u16,
    /// Maximum audio jitter observed during the window (milliseconds).
    pub max_audio_jitter_ms: u16,
    /// Average audio/video drift over the window (milliseconds, signed).
    pub avg_av_drift_ms: i16,
    /// Maximum absolute audio/video drift observed (milliseconds).
    pub max_av_drift_ms: u16,
    /// Exponentially weighted moving average of the drift (milliseconds, signed).
    pub ewma_av_drift_ms: i16,
    /// Count of samples exceeding the configured sync threshold within the window.
    pub violation_count: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct TelemetrySecurityStats {
    pub suite: EncryptionSuite,
    pub rekeys: u32,
    pub gck_rotations: u32,
    pub last_content_key_id: Option<u64>,
    pub last_content_key_valid_from: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct TelemetryEnergyStats {
    pub segment: u64,
    /// Encoder power draw reported in milliwatts.
    pub encoder_milliwatts: u32,
    /// Decoder power draw reported in milliwatts.
    pub decoder_milliwatts: u32,
}

macro_rules! impl_decode_from_slice_via_archived {
    ($($ty:ty),* $(,)?) => {
        $(
            impl<'a> DecodeFromSlice<'a> for $ty {
                fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), CoreError> {
                    norito_core::decode_field_canonical::<$ty>(bytes)
                }
            }
        )*
    };
}

impl_decode_from_slice_via_archived!(
    NeuralBundle,
    FecParameters,
    StorageClass,
    FecScheme,
    CapabilityFlags,
    PrivacyCapabilities,
    FeedbackHint,
    LayerFeedback,
    PrivacyRoute,
    PrivacyRelay,
    StreamMetadata,
    AudioTrackSummary,
    ManifestV1,
    SegmentHeader,
    ChunkDescriptor,
    MerkleProof,
    DataAvailabilityProof,
    ControlFrame,
    FeedbackHintFrame,
    KeyUpdate,
    ContentKeyUpdate,
    PrivacyRouteUpdate,
    CapabilityRole,
    CapabilityReport,
    CapabilityAck,
    EncryptionSuite,
    AudioCapability,
    Resolution,
    AudioLayout,
    AudioFrame,
    PrivacyBucketGranularity,
    TicketCapabilities,
    TicketPolicy,
    StreamingTicket,
    TicketRevocation,
    SyncDiagnostics,
    ReceiverReport,
    TelemetryEvent,
);

/// Frequency table for a single rANS symbol group.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct RansGroupTableV1 {
    /// Bit width (`log2(group_size)`) covered by this table.
    #[norito(default = "default_group_width_bits")]
    pub width_bits: u8,
    /// Number of symbols covered by this table.
    pub group_size: u16,
    /// Precision bits used when normalising frequencies.
    pub precision_bits: u8,
    /// Normalised symbol frequencies.
    pub frequencies: Vec<u16>,
    /// Cumulative distribution (CDF) for the symbol group.
    pub cumulative: Vec<u32>,
}

/// Deterministic rANS table set body.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct RansTablesBodyV1 {
    /// Seed used to derive the frequency tables.
    pub seed: u64,
    /// Maximum bundle width (in bits) covered by this artefact.
    #[norito(default = "default_bundle_width")]
    pub bundle_width: u8,
    /// Frequency/cumulative tables per symbol group.
    pub groups: Vec<RansGroupTableV1>,
}

const fn default_bundle_width() -> u8 {
    0
}

const fn default_group_width_bits() -> u8 {
    0
}

/// Manifest describing deterministic rANS tables and metadata.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct RansTablesV1 {
    /// Manifest version (currently `1`).
    pub version: u16,
    /// Generation timestamp in unix seconds.
    pub generated_at: Timestamp,
    /// Git commit hash of the generator producing the tables.
    pub generator_commit: String,
    /// SHA-256 checksum of the [`RansTablesBodyV1`] Norito payload.
    pub checksum_sha256: [u8; 32],
    /// Deterministic table body.
    pub body: RansTablesBodyV1,
}

/// Signature algorithms supported for rANS table manifests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[norito(tag = "algorithm")]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub enum SignatureAlgorithm {
    /// Ed25519 signature.
    #[norito(rename = "ed25519")]
    Ed25519,
}

impl json::FastJsonWrite for SignatureAlgorithm {
    fn write_json(&self, out: &mut String) {
        let label = match self {
            SignatureAlgorithm::Ed25519 => "ed25519",
        };
        json::write_json_string(label, out);
    }
}

impl json::JsonDeserialize for SignatureAlgorithm {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "ed25519" => Ok(SignatureAlgorithm::Ed25519),
            other => Err(json::Error::Message(format!(
                "unsupported signature algorithm `{other}`"
            ))),
        }
    }
}

/// Optional signature wrapper for [`RansTablesV1`] payloads.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct RansTablesSignatureV1 {
    /// Signature algorithm identifier.
    pub algorithm: SignatureAlgorithm,
    /// Raw public key bytes matching the signature.
    pub public_key: [u8; 32],
    /// Signature payload bytes.
    pub signature: [u8; 64],
}

/// Signed rANS table artefact optionally including an Ed25519 signature.
#[derive(
    Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
#[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
pub struct SignedRansTablesV1 {
    /// Table manifest and deterministic body.
    pub payload: RansTablesV1,
    /// Optional signature attesting the payload.
    pub signature: Option<RansTablesSignatureV1>,
}

pub mod crypto {
    use chacha20poly1305::{
        ChaCha20Poly1305, XChaCha20Poly1305,
        aead::{Aead, KeyInit, Payload},
    };
    use hkdf::Hkdf;
    use sha3::Sha3_256;
    use thiserror::Error;

    use super::{CapabilityRole, ContentKeyUpdate, EncryptionSuite, Hash, KeyUpdate};

    const STS_SALT: &[u8] = b"nsc-sts";
    const STS_ROOT_LABEL: &[u8] = b"nsc-sts-root";
    const STS_SEND_LABEL: &[u8] = b"nsc-sts-send";
    const CEK_LABEL: &[u8] = b"nsc-cek";
    const NONCE_LABEL: &[u8] = b"nsc-nonce";
    const GCK_AAD_LABEL: &[u8] = b"nsc-gck";

    /// Errors emitted by NSC crypto helpers.
    #[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
    pub enum CryptoError {
        #[error("unsupported encryption suite")]
        UnsupportedSuite,
        #[error("hkdf expansion failed")]
        HkdfExpand,
        #[error("invalid ephemeral public key length (expected {expected}, found {found})")]
        InvalidEphemeralPublicKey { expected: usize, found: usize },
        #[error("nonce length mismatch: expected {expected}, found {found}")]
        InvalidNonceLength { expected: usize, found: usize },
        #[error("aead operation failed")]
        AeadFailure,
        #[error("key counter must be strictly increasing (previous {previous}, found {found})")]
        NonMonotonicKeyCounter { previous: u64, found: u64 },
        #[error("encryption suite changed from {expected:?} to {found:?}")]
        SuiteChanged {
            expected: EncryptionSuite,
            found: EncryptionSuite,
        },
        #[error("content key id must increase (previous {previous}, found {found})")]
        ContentKeyRegression { previous: u64, found: u64 },
        #[error("content key update must not carry an empty wrapped key")]
        InvalidWrappedKey,
        #[error("content key valid_from must advance (previous {previous}, found {found})")]
        InvalidValidFrom { previous: u64, found: u64 },
    }

    /// Session transport keys derived for a given endpoint role.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct TransportKeys {
        pub send: [u8; 32],
        pub recv: [u8; 32],
    }

    fn gck_associated_data(content_key_id: u64, valid_from_segment: u64) -> [u8; 23] {
        let mut ad = [0u8; 23];
        ad[..GCK_AAD_LABEL.len()].copy_from_slice(GCK_AAD_LABEL);
        ad[GCK_AAD_LABEL.len()..GCK_AAD_LABEL.len() + 8]
            .copy_from_slice(&content_key_id.to_le_bytes());
        ad[GCK_AAD_LABEL.len() + 8..].copy_from_slice(&valid_from_segment.to_le_bytes());
        ad
    }

    /// Track monotonic key update counters and negotiated suite.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct KeyUpdateState {
        last_counter: Option<u64>,
        suite: Option<EncryptionSuite>,
    }

    /// Persistable representation of [`KeyUpdateState`].
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct KeyUpdateSnapshot {
        /// Encryption suite negotiated for the session.
        pub suite: EncryptionSuite,
        /// Highest key counter accepted so far.
        pub last_counter: u64,
    }

    impl KeyUpdateState {
        pub fn record(&mut self, frame: &KeyUpdate) -> Result<(), CryptoError> {
            if let Some(prev) = self.last_counter
                && frame.key_counter <= prev
            {
                return Err(CryptoError::NonMonotonicKeyCounter {
                    previous: prev,
                    found: frame.key_counter,
                });
            }
            if let Some(suite) = self.suite {
                if suite != frame.suite {
                    return Err(CryptoError::SuiteChanged {
                        expected: suite,
                        found: frame.suite,
                    });
                }
            } else {
                self.suite = Some(frame.suite);
            }
            self.last_counter = Some(frame.key_counter);
            Ok(())
        }

        pub fn suite(&self) -> Option<&EncryptionSuite> {
            self.suite.as_ref()
        }

        pub fn last_counter(&self) -> Option<u64> {
            self.last_counter
        }

        pub fn restore(&mut self, last_counter: Option<u64>, suite: Option<EncryptionSuite>) {
            self.last_counter = last_counter;
            self.suite = suite;
        }

        /// Produce a snapshot capturing the negotiated suite and last accepted counter.
        #[must_use]
        pub fn snapshot(&self) -> Option<KeyUpdateSnapshot> {
            let suite = self.suite?;
            let last_counter = self.last_counter?;
            Some(KeyUpdateSnapshot {
                suite,
                last_counter,
            })
        }

        /// Rehydrate a [`KeyUpdateState`] from a snapshot.
        #[must_use]
        pub fn from_snapshot(snapshot: KeyUpdateSnapshot) -> Self {
            let mut state = Self::default();
            state.restore(Some(snapshot.last_counter), Some(snapshot.suite));
            state
        }
    }

    /// Track content key rotations.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct ContentKeyState {
        last_id: Option<u64>,
        last_valid_from: Option<u64>,
    }

    /// Persistable representation of [`ContentKeyState`].
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct ContentKeySnapshot {
        /// Highest content key identifier accepted so far.
        pub last_id: u64,
        /// Highest valid-from segment observed so far.
        pub last_valid_from: u64,
    }

    impl ContentKeyState {
        pub fn record(&mut self, update: &ContentKeyUpdate) -> Result<(), CryptoError> {
            if update.gck_wrapped.is_empty() {
                return Err(CryptoError::InvalidWrappedKey);
            }
            if let Some(prev) = self.last_id
                && update.content_key_id <= prev
            {
                return Err(CryptoError::ContentKeyRegression {
                    previous: prev,
                    found: update.content_key_id,
                });
            }
            if let Some(prev_valid) = self.last_valid_from
                && update.valid_from_segment < prev_valid
            {
                return Err(CryptoError::InvalidValidFrom {
                    previous: prev_valid,
                    found: update.valid_from_segment,
                });
            }
            self.last_id = Some(update.content_key_id);
            self.last_valid_from = Some(update.valid_from_segment);
            Ok(())
        }

        pub fn last_id(&self) -> Option<u64> {
            self.last_id
        }

        pub fn last_valid_from(&self) -> Option<u64> {
            self.last_valid_from
        }

        pub fn restore(&mut self, last_id: Option<u64>, last_valid_from: Option<u64>) {
            self.last_id = last_id;
            self.last_valid_from = last_valid_from;
        }

        /// Produce a snapshot capturing the last accepted identifiers.
        #[must_use]
        pub fn snapshot(&self) -> Option<ContentKeySnapshot> {
            let last_id = self.last_id?;
            let last_valid_from = self.last_valid_from?;
            Some(ContentKeySnapshot {
                last_id,
                last_valid_from,
            })
        }

        /// Rehydrate a [`ContentKeyState`] from a snapshot.
        #[must_use]
        pub fn from_snapshot(snapshot: ContentKeySnapshot) -> Self {
            let mut state = Self::default();
            state.restore(Some(snapshot.last_id), Some(snapshot.last_valid_from));
            state
        }
    }

    fn role_tag_send(role: CapabilityRole) -> u8 {
        match role {
            CapabilityRole::Publisher => 0x01,
            CapabilityRole::Viewer => 0x02,
        }
    }

    fn opposite_role(role: CapabilityRole) -> CapabilityRole {
        match role {
            CapabilityRole::Publisher => CapabilityRole::Viewer,
            CapabilityRole::Viewer => CapabilityRole::Publisher,
        }
    }

    fn hkdf_expand_with_label(
        hk: &Hkdf<Sha3_256>,
        label: &[u8],
        suffix: &[u8],
        out: &mut [u8],
    ) -> Result<(), CryptoError> {
        let mut info = Vec::with_capacity(label.len() + suffix.len());
        info.extend_from_slice(label);
        info.extend_from_slice(suffix);
        hk.expand(&info, out).map_err(|_| CryptoError::HkdfExpand)
    }

    fn hkdf_from_prk(prk: &[u8; 32]) -> Hkdf<Sha3_256> {
        // `from_prk` panics only if length < hash_len; 32 bytes meets Sha3-256 requirements.
        Hkdf::<Sha3_256>::from_prk(prk).expect("prk length must equal digest output")
    }

    fn suite_nonce_len(suite: &EncryptionSuite) -> usize {
        match suite {
            EncryptionSuite::X25519ChaCha20Poly1305(_) => 12,
            EncryptionSuite::Kyber768XChaCha20Poly1305(_) => 24,
        }
    }

    /// Return the nonce length required by the selected encryption suite.
    pub fn nonce_len_for_suite(suite: &EncryptionSuite) -> usize {
        suite_nonce_len(suite)
    }

    fn chunk_associated_data(segment_number: u64, chunk_id: u16, chunk_root: &Hash) -> [u8; 42] {
        let mut ad = [0u8; 42];
        ad[..8].copy_from_slice(&segment_number.to_le_bytes());
        ad[8..10].copy_from_slice(&chunk_id.to_le_bytes());
        ad[10..].copy_from_slice(chunk_root);
        ad
    }

    /// Derive the session transport secret root from the shared secret output of the handshake.
    pub fn derive_sts_root(shared_secret: &[u8]) -> Result<[u8; 32], CryptoError> {
        let hk = Hkdf::<Sha3_256>::new(Some(STS_SALT), shared_secret);
        let mut root = [0u8; 32];
        hk.expand(STS_ROOT_LABEL, &mut root)
            .map_err(|_| CryptoError::HkdfExpand)?;
        Ok(root)
    }

    /// Derive directional transport keys for the provided endpoint role.
    pub fn derive_transport_keys_for_role(
        shared_secret: &[u8],
        role: CapabilityRole,
    ) -> Result<TransportKeys, CryptoError> {
        let sts_root = derive_sts_root(shared_secret)?;
        derive_transport_keys_from_sts_root(&sts_root, role)
    }

    /// Re-derive transport keys given a previously computed STS root.
    pub fn derive_transport_keys_from_sts_root(
        sts_root: &[u8; 32],
        role: CapabilityRole,
    ) -> Result<TransportKeys, CryptoError> {
        let hk = hkdf_from_prk(sts_root);
        let mut send = [0u8; 32];
        let mut recv = [0u8; 32];
        hkdf_expand_with_label(&hk, STS_SEND_LABEL, &[role_tag_send(role)], &mut send)?;
        hkdf_expand_with_label(
            &hk,
            STS_SEND_LABEL,
            &[role_tag_send(opposite_role(role))],
            &mut recv,
        )?;
        Ok(TransportKeys { send, recv })
    }

    /// Derive the per-segment content encryption key from the GCK.
    pub fn derive_content_key(
        gck: &[u8; 32],
        segment_number: u64,
    ) -> Result<[u8; 32], CryptoError> {
        let hk = hkdf_from_prk(gck);
        let mut cek = [0u8; 32];
        let mut suffix = [0u8; 8];
        suffix.copy_from_slice(&segment_number.to_le_bytes());
        hkdf_expand_with_label(&hk, CEK_LABEL, &suffix, &mut cek)?;
        Ok(cek)
    }

    /// Derive the per-chunk AEAD nonce from the manifest salt.
    pub fn derive_chunk_nonce(
        nonce_salt: &[u8; 32],
        chunk_id: u16,
        suite: &EncryptionSuite,
    ) -> Result<Vec<u8>, CryptoError> {
        let hk = hkdf_from_prk(nonce_salt);
        let nonce_len = suite_nonce_len(suite);
        let mut nonce = vec![0u8; nonce_len];
        let suffix = chunk_id.to_le_bytes();
        hkdf_expand_with_label(&hk, NONCE_LABEL, &suffix, &mut nonce)?;
        Ok(nonce)
    }

    #[inline]
    fn chacha_key_from_bytes(bytes: &[u8; 32]) -> chacha20poly1305::Key {
        (*bytes).into()
    }

    #[inline]
    fn chacha_nonce_from_slice(bytes: &[u8]) -> Result<chacha20poly1305::Nonce, CryptoError> {
        let arr: [u8; 12] = bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidNonceLength {
                expected: 12,
                found: bytes.len(),
            })?;
        Ok(arr.into())
    }

    #[inline]
    fn xchacha_nonce_from_slice(bytes: &[u8]) -> Result<chacha20poly1305::XNonce, CryptoError> {
        let arr: [u8; 24] = bytes
            .try_into()
            .map_err(|_| CryptoError::InvalidNonceLength {
                expected: 24,
                found: bytes.len(),
            })?;
        Ok(arr.into())
    }

    /// Encrypt a chunk payload for the selected suite.
    pub fn encrypt_chunk(
        suite: &EncryptionSuite,
        content_key: &[u8; 32],
        nonce: &[u8],
        segment_number: u64,
        chunk_id: u16,
        chunk_root: &Hash,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let ad = chunk_associated_data(segment_number, chunk_id, chunk_root);
        match suite {
            EncryptionSuite::X25519ChaCha20Poly1305(_) => {
                let key = chacha_key_from_bytes(content_key);
                let nonce = chacha_nonce_from_slice(nonce)?;
                let cipher = ChaCha20Poly1305::new(&key);
                cipher
                    .encrypt(
                        &nonce,
                        Payload {
                            msg: plaintext,
                            aad: &ad,
                        },
                    )
                    .map_err(|_| CryptoError::AeadFailure)
            }
            EncryptionSuite::Kyber768XChaCha20Poly1305(_) => {
                let key = chacha_key_from_bytes(content_key);
                let nonce = xchacha_nonce_from_slice(nonce)?;
                let cipher = XChaCha20Poly1305::new(&key);
                cipher
                    .encrypt(
                        &nonce,
                        Payload {
                            msg: plaintext,
                            aad: &ad,
                        },
                    )
                    .map_err(|_| CryptoError::AeadFailure)
            }
        }
    }

    /// Decrypt a chunk payload for the selected suite.
    pub fn decrypt_chunk(
        suite: &EncryptionSuite,
        content_key: &[u8; 32],
        nonce: &[u8],
        segment_number: u64,
        chunk_id: u16,
        chunk_root: &Hash,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let ad = chunk_associated_data(segment_number, chunk_id, chunk_root);
        match suite {
            EncryptionSuite::X25519ChaCha20Poly1305(_) => {
                let key = chacha_key_from_bytes(content_key);
                let nonce = chacha_nonce_from_slice(nonce)?;
                let cipher = ChaCha20Poly1305::new(&key);
                cipher
                    .decrypt(
                        &nonce,
                        Payload {
                            msg: ciphertext,
                            aad: &ad,
                        },
                    )
                    .map_err(|_| CryptoError::AeadFailure)
            }
            EncryptionSuite::Kyber768XChaCha20Poly1305(_) => {
                let key = chacha_key_from_bytes(content_key);
                let nonce = xchacha_nonce_from_slice(nonce)?;
                let cipher = XChaCha20Poly1305::new(&key);
                cipher
                    .decrypt(
                        &nonce,
                        Payload {
                            msg: ciphertext,
                            aad: &ad,
                        },
                    )
                    .map_err(|_| CryptoError::AeadFailure)
            }
        }
    }

    /// Wrap a Group Content Key (GCK) using the negotiated transport send key and explicit nonce.
    ///
    /// The returned vector concatenates `nonce || ciphertext`, matching the payload layout used in
    /// [`ContentKeyUpdate::gck_wrapped`].
    pub fn wrap_gck(
        suite: &EncryptionSuite,
        transport_send_key: &[u8; 32],
        nonce: &[u8],
        gck_plaintext: &[u8],
        content_key_id: u64,
        valid_from_segment: u64,
    ) -> Result<Vec<u8>, CryptoError> {
        let aad = gck_associated_data(content_key_id, valid_from_segment);
        let ciphertext = match suite {
            EncryptionSuite::X25519ChaCha20Poly1305(_) => {
                let key = chacha_key_from_bytes(transport_send_key);
                let nonce = chacha_nonce_from_slice(nonce)?;
                let cipher = ChaCha20Poly1305::new(&key);
                cipher
                    .encrypt(
                        &nonce,
                        Payload {
                            msg: gck_plaintext,
                            aad: &aad,
                        },
                    )
                    .map_err(|_| CryptoError::AeadFailure)?
            }
            EncryptionSuite::Kyber768XChaCha20Poly1305(_) => {
                let key = chacha_key_from_bytes(transport_send_key);
                let nonce = xchacha_nonce_from_slice(nonce)?;
                let cipher = XChaCha20Poly1305::new(&key);
                cipher
                    .encrypt(
                        &nonce,
                        Payload {
                            msg: gck_plaintext,
                            aad: &aad,
                        },
                    )
                    .map_err(|_| CryptoError::AeadFailure)?
            }
        };
        let mut out = Vec::with_capacity(nonce.len() + ciphertext.len());
        out.extend_from_slice(nonce);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    /// Unwrap a Group Content Key (GCK) using the transport receive key and inline nonce.
    pub fn unwrap_gck(
        suite: &EncryptionSuite,
        transport_recv_key: &[u8; 32],
        nonce: &[u8],
        ciphertext: &[u8],
        content_key_id: u64,
        valid_from_segment: u64,
    ) -> Result<Vec<u8>, CryptoError> {
        let aad = gck_associated_data(content_key_id, valid_from_segment);
        match suite {
            EncryptionSuite::X25519ChaCha20Poly1305(_) => {
                let key = chacha_key_from_bytes(transport_recv_key);
                let nonce = chacha_nonce_from_slice(nonce)?;
                let cipher = ChaCha20Poly1305::new(&key);
                cipher
                    .decrypt(
                        &nonce,
                        Payload {
                            msg: ciphertext,
                            aad: &aad,
                        },
                    )
                    .map_err(|_| CryptoError::AeadFailure)
            }
            EncryptionSuite::Kyber768XChaCha20Poly1305(_) => {
                let key = chacha_key_from_bytes(transport_recv_key);
                let nonce = xchacha_nonce_from_slice(nonce)?;
                let cipher = XChaCha20Poly1305::new(&key);
                cipher
                    .decrypt(
                        &nonce,
                        Payload {
                            msg: ciphertext,
                            aad: &aad,
                        },
                    )
                    .map_err(|_| CryptoError::AeadFailure)
            }
        }
    }

    /// Build the associated data commitments for a ciphertext batch.
    pub fn chunk_commitments_for_ciphertexts(
        segment_number: u64,
        payloads: &[(u16, &[u8])],
    ) -> Vec<Hash> {
        payloads
            .iter()
            .map(|(chunk_id, payload)| {
                super::chunk::chunk_leaf_hash(segment_number, *chunk_id, payload)
            })
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::streaming::{
            CapabilityRole, ContentKeyUpdate, EncryptionSuite, Hash, KeyUpdate, Signature,
        };

        fn sample_suite() -> EncryptionSuite {
            EncryptionSuite::X25519ChaCha20Poly1305([0xAA; 32])
        }

        fn sample_suite_xchacha() -> EncryptionSuite {
            EncryptionSuite::Kyber768XChaCha20Poly1305([0xBB; 32])
        }

        fn sample_hash(seed: u8) -> Hash {
            let mut h = [0u8; 32];
            h.fill(seed);
            h
        }

        fn sample_signature(seed: u8) -> Signature {
            let mut sig = [0u8; 64];
            sig.fill(seed);
            sig
        }

        #[test]
        fn transport_keys_deterministic() {
            let secret = b"shared secret demo";
            let publisher_keys =
                derive_transport_keys_for_role(secret, CapabilityRole::Publisher).unwrap();
            let viewer_keys =
                derive_transport_keys_for_role(secret, CapabilityRole::Viewer).unwrap();
            assert_eq!(publisher_keys.send, viewer_keys.recv);
            assert_eq!(publisher_keys.recv, viewer_keys.send);
            assert_ne!(publisher_keys.send, publisher_keys.recv);
            assert_ne!(viewer_keys.send, viewer_keys.recv);
        }

        #[test]
        fn content_key_and_nonce_deterministic() {
            let gck = sample_hash(1);
            let nonce_salt = sample_hash(2);
            let cek_one = derive_content_key(&gck, 77).unwrap();
            let cek_two = derive_content_key(&gck, 77).unwrap();
            assert_eq!(cek_one, cek_two);

            let suite = sample_suite();
            let nonce_one = derive_chunk_nonce(&nonce_salt, 5, &suite).unwrap();
            let nonce_two = derive_chunk_nonce(&nonce_salt, 5, &suite).unwrap();
            assert_eq!(nonce_one, nonce_two);
            assert_eq!(nonce_one.len(), 12);

            let suite_x = sample_suite_xchacha();
            let nonce_x = derive_chunk_nonce(&nonce_salt, 9, &suite_x).unwrap();
            assert_eq!(nonce_x.len(), 24);
        }

        #[test]
        fn encrypt_decrypt_roundtrip_chacha() {
            let suite = sample_suite();
            let gck = sample_hash(4);
            let cek = derive_content_key(&gck, 10).unwrap();
            let nonce_salt = sample_hash(5);
            let nonce = derive_chunk_nonce(&nonce_salt, 1, &suite).unwrap();
            let chunk_root = sample_hash(9);
            let plaintext = b"deterministic chunk payload";
            let ciphertext =
                encrypt_chunk(&suite, &cek, &nonce, 42, 1, &chunk_root, plaintext).unwrap();
            assert_ne!(ciphertext, plaintext);
            let decrypted =
                decrypt_chunk(&suite, &cek, &nonce, 42, 1, &chunk_root, &ciphertext).unwrap();
            assert_eq!(decrypted, plaintext);
        }

        #[test]
        fn encrypt_decrypt_roundtrip_xchacha() {
            let suite = sample_suite_xchacha();
            let gck = sample_hash(7);
            let cek = derive_content_key(&gck, 55).unwrap();
            let nonce_salt = sample_hash(8);
            let nonce = derive_chunk_nonce(&nonce_salt, 3, &suite).unwrap();
            let chunk_root = sample_hash(10);
            let payload = b"extended nonce payload";
            let ciphertext =
                encrypt_chunk(&suite, &cek, &nonce, 99, 3, &chunk_root, payload).unwrap();
            let decrypted =
                decrypt_chunk(&suite, &cek, &nonce, 99, 3, &chunk_root, &ciphertext).unwrap();
            assert_eq!(decrypted, payload);
        }

        #[test]
        fn key_update_state_enforces_monotonicity_and_suite() {
            let mut state = KeyUpdateState::default();
            let mut frame = KeyUpdate {
                session_id: sample_hash(11),
                suite: sample_suite(),
                protocol_version: 1,
                pub_ephemeral: vec![0; 32],
                key_counter: 1,
                signature: sample_signature(12),
            };
            state.record(&frame).unwrap();
            assert_eq!(state.last_counter(), Some(1));
            assert!(state.suite().is_some());

            frame.key_counter = 2;
            state.record(&frame).unwrap();

            frame.key_counter = 2;
            let err = state.record(&frame).expect_err("non-monotonic counter");
            assert!(matches!(
                err,
                CryptoError::NonMonotonicKeyCounter {
                    previous: 2,
                    found: 2
                }
            ));

            let mut other_frame = frame.clone();
            other_frame.key_counter = 3;
            other_frame.suite = sample_suite_xchacha();
            let err = state
                .record(&other_frame)
                .expect_err("suite change rejected");
            assert!(matches!(err, CryptoError::SuiteChanged { .. }));
        }

        #[test]
        fn content_key_state_enforces_rotation_rules() {
            let mut state = ContentKeyState::default();
            let mut update = ContentKeyUpdate {
                content_key_id: 5,
                gck_wrapped: vec![1, 2, 3],
                valid_from_segment: 20,
            };
            state.record(&update).unwrap();

            update.content_key_id = 5;
            let err = state.record(&update).expect_err("id must increase");
            assert!(matches!(
                err,
                CryptoError::ContentKeyRegression {
                    previous: 5,
                    found: 5
                }
            ));

            update.content_key_id = 6;
            update.valid_from_segment = 10;
            let err = state.record(&update).expect_err("valid_from must advance");
            assert!(matches!(
                err,
                CryptoError::InvalidValidFrom {
                    previous: 20,
                    found: 10
                }
            ));

            let err = state
                .record(&ContentKeyUpdate {
                    content_key_id: 7,
                    gck_wrapped: Vec::new(),
                    valid_from_segment: 30,
                })
                .expect_err("empty gck");
            assert!(matches!(err, CryptoError::InvalidWrappedKey));
        }

        #[test]
        fn gck_wrap_unwrap_roundtrip() {
            let suite = sample_suite();
            let transport_key = sample_hash(33);
            let nonce_len = nonce_len_for_suite(&suite);
            let mut nonce = vec![0u8; nonce_len];
            for (idx, byte) in nonce.iter_mut().enumerate() {
                *byte = (idx as u8).wrapping_mul(9);
            }
            let gck = sample_hash(44);
            let wrapped = wrap_gck(&suite, &transport_key, &nonce, &gck, 9, 128).expect("wrap");
            assert_eq!(wrapped.len(), nonce_len + gck.len() + 16);
            let (nonce_part, ciphertext_part) = wrapped.split_at(nonce_len);
            assert_eq!(nonce_part, nonce.as_slice());
            let unwrapped = unwrap_gck(&suite, &transport_key, nonce_part, ciphertext_part, 9, 128)
                .expect("unwrap");
            assert_eq!(unwrapped, gck);
        }
    }
}

pub mod chunk {
    use thiserror::Error;

    use super::{AudioCodecError, Hash, MerkleProof, saturating_usize_to_u32};
    use crate::streaming::codec::{
        Chroma420Frame, EncodedSegment, FRAME_HEADER_LEN, FrameDimensions, FrameType, SegmentError,
        decode_block_rle, dequantize_coeffs, inverse_dct, predictor_block, verify_segment,
        write_reconstructed_block,
    };

    const LEAF_DOMAIN: &[u8] = b"nsc_ct_leaf";
    const NODE_DOMAIN: &[u8] = b"nsc_ct_node";
    const STORAGE_DOMAIN: &[u8] = b"nsc_storage";
    const DA_DOMAIN: &[u8] = b"nsc_da";

    /// Bounds violation details returned by [`ChunkError::IndexOutOfBounds`].
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct ChunkIndexBounds {
        pub index: u32,
        pub len: u32,
    }

    /// Errors emitted by chunk hashing and Merkle utilities.
    #[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub enum ChunkError {
        #[error("merkle tree requires at least one leaf")]
        EmptyTree,
        #[error(
            "chunk index {index} out of bounds for {len} leaves",
            index = .0.index,
            len = .0.len
        )]
        IndexOutOfBounds(ChunkIndexBounds),
        #[error("chunk id list must be strictly ascending")]
        UnsortedChunkIds,
    }

    /// Compute the leaf hash for a chunk ciphertext.
    pub fn chunk_leaf_hash(segment_number: u64, chunk_id: u16, ciphertext: &[u8]) -> Hash {
        let mut hasher = domain_hasher(LEAF_DOMAIN);
        hasher.update(&segment_number.to_le_bytes());
        hasher.update(&chunk_id.to_le_bytes());
        hasher.update(ciphertext);
        finalize_hash(hasher)
    }

    /// Compute chunk commitments for a slice of chunk payloads.
    pub fn chunk_commitments(segment_number: u64, chunks: &[(u16, &[u8])]) -> Vec<Hash> {
        chunks
            .iter()
            .map(|(id, payload)| chunk_leaf_hash(segment_number, *id, payload))
            .collect()
    }

    /// Compute the Merkle root for a set of chunk commitments.
    pub fn merkle_root(leaves: &[Hash]) -> Result<Hash, ChunkError> {
        if leaves.is_empty() {
            return Err(ChunkError::EmptyTree);
        }
        if leaves.len() == 1 {
            return Ok(leaves[0]);
        }

        let mut level: Vec<Hash> = leaves.to_vec();
        while level.len() > 1 {
            level = next_parent_level(&level);
        }
        Ok(level[0])
    }

    /// Build a Merkle proof for a given leaf index.
    pub fn merkle_proof(
        leaves: &[Hash],
        index: usize,
        chunk_id: u16,
    ) -> Result<MerkleProof, ChunkError> {
        if leaves.is_empty() {
            return Err(ChunkError::EmptyTree);
        }
        if index >= leaves.len() {
            return Err(ChunkError::IndexOutOfBounds(ChunkIndexBounds {
                index: saturating_usize_to_u32(index),
                len: saturating_usize_to_u32(leaves.len()),
            }));
        }

        let mut current = leaves.to_vec();
        let mut siblings = Vec::new();
        let mut directions = Vec::new();
        let mut idx = index;

        while current.len() > 1 {
            let is_right = idx % 2 == 1;
            let sibling_idx = if is_right {
                idx - 1
            } else if idx + 1 < current.len() {
                idx + 1
            } else {
                idx
            };
            siblings.push(current[sibling_idx]);
            directions.push(is_right);
            current = next_parent_level(&current);
            idx /= 2;
        }

        Ok(MerkleProof {
            chunk_id,
            sibling_hashes: siblings,
            directions,
        })
    }

    /// Verify a Merkle proof against an expected root.
    pub fn verify_merkle_proof(leaf: &Hash, proof: &MerkleProof, expected_root: &Hash) -> bool {
        if proof.sibling_hashes.len() != proof.directions.len() {
            return false;
        }

        let mut node = *leaf;
        for (sibling, is_left) in proof.sibling_hashes.iter().zip(proof.directions.iter()) {
            node = if *is_left {
                parent_hash(sibling, &node)
            } else {
                parent_hash(&node, sibling)
            };
        }
        node == *expected_root
    }

    /// Compute the data-availability storage commitment.
    pub fn storage_commitment(
        segment_number: u64,
        content_key_id: u64,
        chunk_root: &Hash,
        chunk_ids: &[u16],
    ) -> Result<Hash, ChunkError> {
        if !is_strictly_ascending(chunk_ids) {
            return Err(ChunkError::UnsortedChunkIds);
        }
        let mut hasher = domain_hasher(STORAGE_DOMAIN);
        hasher.update(&segment_number.to_le_bytes());
        hasher.update(&content_key_id.to_le_bytes());
        hasher.update(chunk_root);
        for id in chunk_ids {
            hasher.update(&id.to_le_bytes());
        }
        Ok(finalize_hash(hasher))
    }

    /// Compute the data availability proof root helper per spec (`nsc_da`).
    pub fn data_availability_root(
        segment_number: u64,
        content_key_id: u64,
        chunk_root: &Hash,
        chunk_ids: &[u16],
    ) -> Result<Hash, ChunkError> {
        if !is_strictly_ascending(chunk_ids) {
            return Err(ChunkError::UnsortedChunkIds);
        }
        let mut hasher = domain_hasher(DA_DOMAIN);
        hasher.update(&segment_number.to_le_bytes());
        hasher.update(&content_key_id.to_le_bytes());
        hasher.update(chunk_root);
        for id in chunk_ids {
            hasher.update(&id.to_le_bytes());
        }
        Ok(finalize_hash(hasher))
    }

    fn next_parent_level(level: &[Hash]) -> Vec<Hash> {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let left = pair[0];
            let right = if pair.len() == 2 { pair[1] } else { pair[0] };
            next.push(parent_hash(&left, &right));
        }
        next
    }

    fn parent_hash(left: &Hash, right: &Hash) -> Hash {
        let mut hasher = domain_hasher(NODE_DOMAIN);
        hasher.update(left);
        hasher.update(right);
        finalize_hash(hasher)
    }

    fn finalize_hash(hasher: blake3::Hasher) -> Hash {
        let digest = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(digest.as_bytes());
        out
    }

    fn domain_hasher(domain: &[u8]) -> blake3::Hasher {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain);
        hasher
    }

    fn is_strictly_ascending(ids: &[u16]) -> bool {
        ids.windows(2).all(|w| w[0] < w[1])
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct FrameLengthMismatch {
        pub expected: u32,
        pub actual: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct FrameIndexMismatchInfo {
        pub expected: u32,
        pub found: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct FramePtsMismatchInfo {
        pub index: u32,
        pub expected: u64,
        pub found: u64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct UnalignedDimensionsInfo {
        pub width: u16,
        pub height: u16,
        pub block_size: u8,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct BlockCountMismatchInfo {
        pub expected: u32,
        pub found: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct FrameCountOverflowInfo {
        pub max: u32,
        pub found: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct AudioSampleCountMismatchInfo {
        pub expected: u64,
        pub found: u64,
    }

    /// Error raised when the encoded chroma payload is shorter than advertised.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct ChromaPayloadTruncatedInfo;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct ChromaDimensionsNotEvenInfo {
        pub width: u16,
        pub height: u16,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct AudioFrameCadenceMismatchInfo {
        pub expected: u16,
        pub found: u16,
        pub sample_rate: u32,
        pub frame_duration_ns: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct AudioFrameCadenceOverflowInfo {
        pub expected: u64,
        pub sample_rate: u32,
        pub frame_duration_ns: u32,
    }

    #[derive(Debug, Error)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub enum CodecError {
        #[error(transparent)]
        Segment(#[from] SegmentError),
        #[error(transparent)]
        Audio(#[from] AudioCodecError),
        #[error(
            "frame payload has wrong length: expected {expected}, got {actual}",
            expected = .0.expected,
            actual = .0.actual
        )]
        InvalidFrameLength(FrameLengthMismatch),
        #[error("chunk payload truncated")]
        ChunkTooShort,
        #[error(
            "frame index mismatch: expected {expected}, found {found}",
            expected = .0.expected,
            found = .0.found
        )]
        FrameIndexMismatch(FrameIndexMismatchInfo),
        #[error(
            "frame pts mismatch at index {index}: expected {expected}, found {found}",
            index = .0.index,
            expected = .0.expected,
            found = .0.found
        )]
        FramePtsMismatch(FramePtsMismatchInfo),
        #[error("frame pts computation overflow at index {0}")]
        FramePtsOverflow(u32),
        #[error(
            "frame dimensions ({width}x{height}) are not divisible by {block_size} pixels",
            width = .0.width,
            height = .0.height,
            block_size = .0.block_size
        )]
        UnalignedDimensions(UnalignedDimensionsInfo),
        #[error(
            "encoded block count mismatch: expected {expected}, found {found}",
            expected = .0.expected,
            found = .0.found
        )]
        BlockCountMismatch(BlockCountMismatchInfo),
        #[error(
            "frame count exceeds u16 range: expected <= {max}, found {found}",
            max = .0.max,
            found = .0.found
        )]
        FrameCountOverflow(FrameCountOverflowInfo),
        #[error("run-length stream overflow while decoding block {0}")]
        RleOverflow(u32),
        #[error("run-length stream truncated while decoding block {0}")]
        TruncatedBlock(u32),
        #[error("unknown frame type id {0}")]
        UnknownFrameType(u8),
        #[error("missing end-of-block marker while decoding block {0}")]
        MissingEndOfBlock(u32),
        #[error("audio track configured but samples missing")]
        AudioTrackMissing,
        #[error("audio track provided but encoder audio is disabled")]
        AudioTrackUnexpected,
        #[error(
            "audio frame cadence mismatch: expected {expected} samples for {sample_rate} Hz/{frame_duration_ns} ns, found {found}",
            expected = .0.expected,
            found = .0.found,
            sample_rate = .0.sample_rate,
            frame_duration_ns = .0.frame_duration_ns
        )]
        AudioFrameCadenceMismatch(AudioFrameCadenceMismatchInfo),
        #[error(
            "audio frame cadence overflow for {sample_rate} Hz/{frame_duration_ns} ns (expected {expected} samples)",
            expected = .0.expected,
            sample_rate = .0.sample_rate,
            frame_duration_ns = .0.frame_duration_ns
        )]
        AudioFrameCadenceOverflow(AudioFrameCadenceOverflowInfo),
        #[error(
            "audio sample count mismatch: expected {expected}, found {found}",
            expected = .0.expected,
            found = .0.found
        )]
        AudioSampleCountMismatch(AudioSampleCountMismatchInfo),
        #[error("chroma payload truncated")]
        ChromaPayloadTruncated(ChromaPayloadTruncatedInfo),
        #[error(
            "chroma 4:2:0 requires even dimensions, got {width}x{height}",
            width = .0.width,
            height = .0.height
        )]
        ChromaDimensionsNotEven(ChromaDimensionsNotEvenInfo),
        #[error("audio sample count overflow during validation")]
        AudioSampleCountOverflow,
        #[error("audio frame count exceeds u16 range")]
        AudioFrameCountOverflow,
        #[error("audio sequence counter overflow")]
        AudioSequenceOverflow,
        #[error("audio timestamp overflow at index {0}")]
        AudioTimestampOverflow(u32),
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct BaselineDecoder {
        dimensions: FrameDimensions,
        frame_duration_ns: u32,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct DecodedFrame {
        pub index: u32,
        pub pts_ns: u64,
        pub luma: Vec<u8>,
        pub chroma: Option<Chroma420Frame>,
    }

    impl BaselineDecoder {
        pub fn new(dimensions: FrameDimensions, frame_duration_ns: u32) -> Self {
            Self {
                dimensions,
                frame_duration_ns,
            }
        }

        pub fn decode_segment(
            &self,
            segment: &EncodedSegment,
        ) -> Result<Vec<DecodedFrame>, CodecError> {
            verify_segment(
                &segment.header,
                &segment.descriptors,
                &segment.chunks,
                segment.audio.as_ref(),
            )?;

            let mut frames = Vec::with_capacity(segment.chunks.len());
            let frame_step = u64::from(self.frame_duration_ns);
            let dims = self.dimensions;
            let aligned_dims = dims.align_to_block();
            let expected_blocks = aligned_dims.block_count();
            let mut previous_frame: Option<Vec<u8>> = None;
            let chroma_dims = FrameDimensions::new(dims.width / 2, dims.height / 2);
            let mut previous_chroma_u: Option<Vec<u8>> = None;
            let mut previous_chroma_v: Option<Vec<u8>> = None;
            for (idx, chunk) in segment.chunks.iter().enumerate() {
                if chunk.len() < FRAME_HEADER_LEN {
                    return Err(CodecError::ChunkTooShort);
                }
                let mut index_bytes = [0u8; 4];
                index_bytes.copy_from_slice(&chunk[..4]);
                let frame_index = u32::from_le_bytes(index_bytes);
                if frame_index != idx as u32 {
                    return Err(CodecError::FrameIndexMismatch(
                        super::chunk::FrameIndexMismatchInfo {
                            expected: idx as u32,
                            found: frame_index,
                        },
                    ));
                }
                let mut pts_bytes = [0u8; 8];
                pts_bytes.copy_from_slice(&chunk[4..12]);
                let pts_ns = u64::from_le_bytes(pts_bytes);
                let delta = frame_step
                    .checked_mul(idx as u64)
                    .ok_or(CodecError::FramePtsOverflow(saturating_usize_to_u32(idx)))?;
                let expected_pts = segment
                    .header
                    .timeline_start_ns
                    .checked_add(delta)
                    .ok_or(CodecError::FramePtsOverflow(saturating_usize_to_u32(idx)))?;
                if pts_ns != expected_pts {
                    return Err(CodecError::FramePtsMismatch(
                        super::chunk::FramePtsMismatchInfo {
                            index: saturating_usize_to_u32(idx),
                            expected: expected_pts,
                            found: pts_ns,
                        },
                    ));
                }
                let frame_type = FrameType::from_byte(chunk[12])?;
                let quantizer = chunk[13];
                let block_count =
                    u16::from_le_bytes(chunk[14..16].try_into().expect("header has enough bytes"));
                if block_count as usize != expected_blocks {
                    return Err(CodecError::BlockCountMismatch(BlockCountMismatchInfo {
                        expected: saturating_usize_to_u32(expected_blocks),
                        found: block_count as u32,
                    }));
                }

                let mut offset = FRAME_HEADER_LEN;
                let mut reconstructed = vec![0u8; aligned_dims.pixel_count()];
                let mut prev_dc = 0i16;
                if frame_type.reset_dc() {
                    prev_dc = 0;
                }
                for block_idx in 0..expected_blocks {
                    let coeffs =
                        decode_block_rle(chunk, &mut offset, &mut prev_dc, block_idx as u32)?;
                    let dequant = dequantize_coeffs(&coeffs, quantizer);
                    let spatial = inverse_dct(&dequant);
                    let predictor = predictor_block(
                        previous_frame.as_deref(),
                        aligned_dims,
                        block_idx,
                        frame_type,
                    );
                    write_reconstructed_block(
                        &mut reconstructed,
                        &spatial,
                        &predictor,
                        aligned_dims,
                        block_idx,
                    );
                }

                #[cfg(feature = "streaming-neural-filter")]
                apply_neural_filter(&mut reconstructed, aligned_dims);

                let cropped =
                    crate::streaming::codec::crop_frame_luma(&reconstructed, dims, aligned_dims);
                let chroma = if chunk.len() > offset {
                    crate::streaming::codec::ensure_chroma_even_dimensions(dims)?;
                    if chunk.len().saturating_sub(offset) < 8 {
                        return Err(CodecError::ChromaPayloadTruncated(
                            ChromaPayloadTruncatedInfo,
                        ));
                    }
                    let mut u_len_bytes = [0u8; 4];
                    u_len_bytes.copy_from_slice(&chunk[offset..offset + 4]);
                    let u_len = u32::from_le_bytes(u_len_bytes) as usize;
                    let mut v_len_bytes = [0u8; 4];
                    v_len_bytes.copy_from_slice(&chunk[offset + 4..offset + 8]);
                    let v_len = u32::from_le_bytes(v_len_bytes) as usize;
                    let chroma_start = offset + 8;
                    let u_end = chroma_start.checked_add(u_len).ok_or(
                        CodecError::ChromaPayloadTruncated(ChromaPayloadTruncatedInfo),
                    )?;
                    let v_end =
                        u_end
                            .checked_add(v_len)
                            .ok_or(CodecError::ChromaPayloadTruncated(
                                ChromaPayloadTruncatedInfo,
                            ))?;
                    if v_end > chunk.len() {
                        return Err(CodecError::ChromaPayloadTruncated(
                            ChromaPayloadTruncatedInfo,
                        ));
                    }
                    let (u_plane, reconstructed_u) = decode_chroma_plane(
                        &chunk[chroma_start..u_end],
                        frame_type,
                        quantizer,
                        chroma_dims,
                        previous_chroma_u.as_deref(),
                    )?;
                    let (v_plane, reconstructed_v) = decode_chroma_plane(
                        &chunk[u_end..v_end],
                        frame_type,
                        quantizer,
                        chroma_dims,
                        previous_chroma_v.as_deref(),
                    )?;
                    previous_chroma_u = Some(reconstructed_u);
                    previous_chroma_v = Some(reconstructed_v);
                    Some(Chroma420Frame::new(dims, u_plane, v_plane)?)
                } else {
                    previous_chroma_u = None;
                    previous_chroma_v = None;
                    None
                };
                frames.push(DecodedFrame {
                    index: frame_index,
                    pts_ns,
                    luma: cropped,
                    chroma,
                });
                previous_frame = Some(reconstructed);
            }
            Ok(frames)
        }
    }

    fn decode_chroma_plane(
        payload: &[u8],
        frame_type: FrameType,
        quantizer: u8,
        dimensions: FrameDimensions,
        previous: Option<&[u8]>,
    ) -> Result<(Vec<u8>, Vec<u8>), CodecError> {
        let aligned = dimensions.align_to_block();
        let expected_blocks = aligned.block_count();
        let mut offset = 0usize;
        let mut reconstructed = vec![0u8; aligned.pixel_count()];
        let mut prev_dc = 0i16;
        if frame_type.reset_dc() {
            prev_dc = 0;
        }
        for block_idx in 0..expected_blocks {
            let coeffs = decode_block_rle(payload, &mut offset, &mut prev_dc, block_idx as u32)?;
            let dequant = dequantize_coeffs(&coeffs, quantizer);
            let spatial = inverse_dct(&dequant);
            let predictor = predictor_block(previous, aligned, block_idx, frame_type);
            write_reconstructed_block(&mut reconstructed, &spatial, &predictor, aligned, block_idx);
        }

        let cropped = crate::streaming::codec::crop_frame_luma(&reconstructed, dimensions, aligned);
        Ok((cropped, reconstructed))
    }

    pub(crate) fn derive_nonce_salt(
        segment_number: u64,
        frame_count: usize,
        chunks: &[Vec<u8>],
    ) -> Hash {
        let mut hasher = domain_hasher(b"nsc_nonce");
        hasher.update(&segment_number.to_le_bytes());
        hasher.update(&(frame_count as u32).to_le_bytes());
        for chunk in chunks {
            hasher.update(&(chunk.len() as u32).to_le_bytes());
            hasher.update(chunk);
        }
        finalize_hash(hasher)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn sample_payload(byte: u8, len: usize) -> Vec<u8> {
            vec![byte; len]
        }

        #[test]
        fn merkle_root_single_leaf_is_leaf() {
            let payload = sample_payload(0xAA, 8);
            let leaf = chunk_leaf_hash(1, 0, &payload);
            let root = merkle_root(&[leaf]).expect("root");
            assert_eq!(root, leaf);
        }

        #[test]
        fn merkle_proof_verifies_for_even_leaves() {
            let ciphertexts = [
                (0u16, sample_payload(0x01, 4)),
                (1u16, sample_payload(0x02, 4)),
                (2u16, sample_payload(0x03, 4)),
                (3u16, sample_payload(0x04, 4)),
            ];
            let payload_refs: Vec<(u16, &[u8])> = ciphertexts
                .iter()
                .map(|(id, data)| (*id, data.as_slice()))
                .collect();
            let leaves = chunk_commitments(7, &payload_refs);
            let root = merkle_root(&leaves).expect("root");
            let proof = merkle_proof(&leaves, 2, 2).expect("proof");
            let leaf = leaves[2];
            assert!(verify_merkle_proof(&leaf, &proof, &root));
        }

        #[test]
        fn merkle_proof_verifies_for_odd_leaves() {
            let ciphertexts = [
                (0u16, sample_payload(0x10, 3)),
                (1u16, sample_payload(0x11, 3)),
                (2u16, sample_payload(0x12, 3)),
            ];
            let payload_refs: Vec<(u16, &[u8])> = ciphertexts
                .iter()
                .map(|(id, data)| (*id, data.as_slice()))
                .collect();
            let leaves = chunk_commitments(9, &payload_refs);
            let root = merkle_root(&leaves).expect("root");
            let proof = merkle_proof(&leaves, 2, 2).expect("proof");
            let leaf = leaves[2];
            assert!(verify_merkle_proof(&leaf, &proof, &root));
        }

        #[test]
        fn merkle_proof_detects_bad_root() {
            let leaves = vec![
                chunk_leaf_hash(1, 0, &sample_payload(1, 2)),
                chunk_leaf_hash(1, 1, &sample_payload(2, 2)),
            ];
            let root = merkle_root(&leaves).expect("root");
            let proof = merkle_proof(&leaves, 0, 0).expect("proof");
            let mut tampered_root = root;
            tampered_root[0] ^= 0xFF;
            assert!(!verify_merkle_proof(&leaves[0], &proof, &tampered_root));
        }

        #[test]
        fn storage_commitment_requires_sorted_ids() {
            let chunk_root = chunk_leaf_hash(5, 0, &sample_payload(0xAA, 1));
            let err = storage_commitment(5, 1, &chunk_root, &[2, 1]).unwrap_err();
            assert!(matches!(err, ChunkError::UnsortedChunkIds));
        }

        #[test]
        fn storage_commitment_matches_spec() {
            let chunk_root = chunk_leaf_hash(5, 0, &sample_payload(0xAA, 4));
            let commitment =
                storage_commitment(5, 12, &chunk_root, &[0, 3, 7]).expect("commitment");
            let proof_root = data_availability_root(5, 12, &chunk_root, &[0, 3, 7]).expect("da");
            assert_ne!(commitment, [0u8; 32]);
            assert_ne!(proof_root, [0u8; 32]);
        }

        #[test]
        fn derive_nonce_salt_varies_with_chunk_payloads() {
            let chunks = vec![sample_payload(0x01, 4), sample_payload(0x02, 8)];
            let salt_a = derive_nonce_salt(9, chunks.len(), &chunks);
            let salt_b = derive_nonce_salt(9, chunks.len(), &chunks);
            assert_eq!(salt_a, salt_b, "nonce salt must be deterministic");

            let mut altered = chunks.clone();
            altered[1][3] ^= 0xFF;
            let salt_c = derive_nonce_salt(9, altered.len(), &altered);
            assert_ne!(salt_a, salt_c, "nonce salt must reflect chunk contents");

            let salt_d = derive_nonce_salt(10, chunks.len(), &chunks);
            assert_ne!(salt_a, salt_d, "nonce salt must capture segment number");
        }
    }
}

pub mod codec {
    use std::{
        collections::{BTreeMap, VecDeque},
        convert::TryInto,
        fs,
        path::Path,
        sync::{Arc, OnceLock},
    };

    use norito_derive::{NoritoDeserialize, NoritoSerialize};
    use sha2::{Digest, Sha256};
    use thiserror::Error;
    use toml::Value as TomlValue;

    use super::{
        AudioCodecError, AudioCodecLayoutMismatchInfo, AudioEncoderSampleCountMismatchInfo,
        AudioFrame, AudioLayout, AudioTrackSummary, BundleAcceleration, Bytes, CapabilityFlags,
        ChunkDescriptor, EncryptionSuite, EntropyMode, FecScheme, FeedbackHint, Hash, ManifestV1,
        Multiaddr, NeuralBundle, NoritoJsonValue, PrivacyRoute, ProfileId, RansGroupTableV1,
        RansTablesBodyV1, RansTablesSignatureV1, RansTablesV1, RdoMode, SegmentAudio,
        SegmentHeader, Signature, SignatureAlgorithm, SignedRansTablesV1, StorageClass,
        StreamMetadata, Timestamp,
        chunk::{
            AudioFrameCadenceMismatchInfo, AudioFrameCadenceOverflowInfo,
            AudioSampleCountMismatchInfo, BlockCountMismatchInfo, ChromaDimensionsNotEvenInfo,
            ChunkError, CodecError, FrameCountOverflowInfo, FrameLengthMismatch, chunk_commitments,
            derive_nonce_salt, merkle_root,
        },
        json, norito_core, saturating_usize_to_u32, saturating_usize_to_u64,
    };
    use crate as norito;

    pub(crate) const BLOCK_SIZE: usize = 8;
    pub(crate) const BLOCK_PIXELS: usize = BLOCK_SIZE * BLOCK_SIZE;
    pub(crate) const FRAME_HEADER_LEN: usize = 16;
    const RLE_EOB: u8 = 0xFF;
    const MAX_ZERO_RUN: usize = 254;
    const RLE_TOKEN_BITS: u64 = 24;
    const RLE_TOKEN_BITS_F: f32 = 24.0;
    const RLE_EOB_BITS_F: f32 = 24.0;
    const RDO_ENERGY_BUCKETS: [u32; RDO_BUCKET_COUNT - 1] = [64, 256, 1024, 4096];
    const NEURAL_FEATURES: usize = 8;
    const NEURAL_HIDDEN: usize = 32;
    const NEURAL_OUTPUT: usize = 4;
    const DEFAULT_NEURAL_SEED: [u8; 32] = *b"nsc-neural-predictor-v1--seed!!!";
    #[cfg(not(feature = "streaming-fixed-point-dct"))]
    const DCT_FACTORS: [[f64; 8]; 8] = [
        [
            0.3535533905932737,
            0.3535533905932737,
            0.3535533905932737,
            0.3535533905932737,
            0.3535533905932737,
            0.3535533905932737,
            0.3535533905932737,
            0.3535533905932737,
        ],
        [
            0.4903926402016152,
            0.4157348061512726,
            0.2777851165098011,
            0.0975451610080642,
            -0.0975451610080641,
            -0.277785116509801,
            -0.4157348061512727,
            -0.4903926402016152,
        ],
        [
            0.4619397662556434,
            0.1913417161825449,
            -0.1913417161825449,
            -0.4619397662556434,
            -0.4619397662556434,
            -0.1913417161825452,
            0.191341716182545,
            0.4619397662556433,
        ],
        [
            0.4157348061512726,
            -0.0975451610080641,
            -0.4903926402016152,
            -0.2777851165098011,
            0.2777851165098009,
            0.4903926402016152,
            0.0975451610080644,
            -0.4157348061512726,
        ],
        [
            0.3535533905932738,
            -0.3535533905932737,
            -0.3535533905932738,
            0.3535533905932737,
            0.3535533905932738,
            -0.3535533905932733,
            -0.3535533905932736,
            0.3535533905932733,
        ],
        [
            0.2777851165098011,
            -0.4903926402016152,
            0.0975451610080642,
            0.4157348061512727,
            -0.4157348061512726,
            -0.097545161008064,
            0.4903926402016153,
            -0.2777851165098008,
        ],
        [
            0.1913417161825449,
            -0.4619397662556434,
            0.4619397662556433,
            -0.1913417161825449,
            -0.1913417161825453,
            0.4619397662556434,
            -0.4619397662556432,
            0.1913417161825448,
        ],
        [
            0.0975451610080642,
            -0.2777851165098011,
            0.4157348061512727,
            -0.4903926402016153,
            0.4903926402016152,
            -0.4157348061512725,
            0.2777851165098008,
            -0.0975451610080643,
        ],
    ];

    #[cfg(feature = "streaming-fixed-point-dct")]
    const DCT_FACTORS_Q15: [[i16; 8]; 8] = [
        [11585, 11585, 11585, 11585, 11585, 11585, 11585, 11585],
        [16069, 13623, 9102, 3196, -3196, -9102, -13623, -16069],
        [15137, 6270, -6270, -15137, -15137, -6270, 6270, 15137],
        [13623, -3196, -16069, -9102, 9102, 16069, 3196, -13623],
        [11585, -11585, -11585, 11585, 11585, -11585, -11585, 11585],
        [9102, -16069, 3196, 13623, -13623, -3196, 16069, -9102],
        [6270, -15137, 15137, -6270, -6270, 15137, -15137, 6270],
        [3196, -9102, 13623, -16069, 16069, -13623, 9102, -3196],
    ];

    #[cfg(feature = "streaming-fixed-point-dct")]
    const DCT_Q_BITS: u32 = 15;
    #[cfg(feature = "streaming-fixed-point-dct")]
    const DCT_SHIFT: u32 = DCT_Q_BITS * 2;
    #[cfg(feature = "streaming-fixed-point-dct")]
    const DCT_ROUND: i64 = 1 << (DCT_SHIFT - 1);

    #[derive(Debug, Error)]
    pub enum BundleTableError {
        #[error("failed to read SignedRansTablesV1 artefact: {0}")]
        Io(#[from] std::io::Error),
        #[error("failed to parse SignedRansTablesV1 TOML: {0}")]
        Toml(#[from] toml::de::Error),
        #[error("invalid SignedRansTablesV1 structure: {0}")]
        InvalidStructure(&'static str),
        #[error("invalid SignedRansTablesV1 payload: {0}")]
        Json(#[from] json::Error),
        #[error("checksum mismatch for SignedRansTablesV1 payload")]
        ChecksumMismatch,
        #[error("missing rANS frequency group for bit length {bit_len}")]
        MissingGroup { bit_len: u8 },
        #[error("invalid rANS frequency group for bit length {bit_len}: {reason}")]
        InvalidGroup { bit_len: u8, reason: &'static str },
    }

    /// Errors returned when decoding bundled rANS streams.
    #[derive(Clone, Debug, Error, PartialEq, Eq)]
    pub enum BundleDecodeError {
        /// Stream too short to contain the serialized ANS state.
        #[error("bundle rANS stream missing serialized state")]
        TruncatedState,
        /// SIMD bundle stream is missing the header or lane length metadata.
        #[error("SIMD bundle stream missing header or lane lengths")]
        InvalidSimdHeader,
        /// Declared stream lengths do not match the payload.
        #[error("bundle rANS stream length mismatch")]
        LengthMismatch,
        /// Table checksum does not match the telemetry-provided checksum.
        #[error("bundle table checksum mismatch: expected {expected:?}, found {found:?}")]
        ChecksumMismatch { expected: Hash, found: Hash },
        /// Ran out of bytes while renormalizing the ANS state.
        #[error("bundle rANS renormalization underflow")]
        RenormalizeUnderflow,
        /// Decoded symbol does not match the recorded bundle bits.
        #[error(
            "bundle symbol mismatch at index {index}: expected {expected:#04x}, found {found:#04x}"
        )]
        SymbolMismatch {
            /// Position within the decoded bundle stream.
            index: u32,
            /// Symbol reconstructed from the bundle record.
            expected: u8,
            /// Symbol produced by the ANS stream.
            found: u8,
        },
    }

    /// Errors encountered while loading or applying a bundle context remap.
    #[derive(Debug, Error)]
    pub enum ContextRemapError {
        /// The remap JSON could not be read from disk.
        #[error("failed to read context remap file: {0}")]
        Io(#[from] std::io::Error),
        /// The remap JSON could not be parsed.
        #[error("failed to parse context remap file: {0}")]
        Json(#[from] json::Error),
        /// The remap JSON is missing required fields.
        #[error("context remap is missing required field: {0}")]
        Missing(&'static str),
        /// A context identifier in the remap was outside the supported range.
        #[error("context remap contains an invalid context id {0}")]
        InvalidContext(u64),
    }

    /// Mapping from raw bundle contexts to a pruned/remapped domain.
    #[derive(Clone, Debug)]
    pub struct BundleContextRemap {
        mapping: BTreeMap<BundleContextId, BundleContextId>,
        escape_context: BundleContextId,
        remapped: u32,
        dropped: u32,
    }

    impl BundleContextRemap {
        /// Construct a new remap with the provided mapping table and escape context.
        pub fn new(
            mapping: BTreeMap<BundleContextId, BundleContextId>,
            escape_context: BundleContextId,
            remapped: u32,
            dropped: u32,
        ) -> Self {
            Self {
                mapping,
                escape_context,
                remapped,
                dropped,
            }
        }

        /// Apply the remap to a raw context, returning the mapped or escape context.
        #[must_use]
        pub fn map(&self, context: BundleContextId) -> BundleContextId {
            *self.mapping.get(&context).unwrap_or(&self.escape_context)
        }

        /// Escape context used for unmapped entries.
        #[must_use]
        pub const fn escape_context(&self) -> BundleContextId {
            self.escape_context
        }

        /// Number of contexts explicitly remapped.
        #[must_use]
        pub const fn remapped_count(&self) -> u32 {
            self.remapped
        }

        /// Number of contexts routed to the escape bucket.
        #[must_use]
        pub const fn dropped_count(&self) -> u32 {
            self.dropped
        }

        fn from_report(value: &NoritoJsonValue) -> Result<Self, ContextRemapError> {
            let escape_context = value
                .get("escape_context")
                .and_then(NoritoJsonValue::as_u64)
                .and_then(|raw| u16::try_from(raw).ok())
                .map(BundleContextId::new)
                .unwrap_or_else(|| BundleContextId::new(u16::MAX));
            let mut mapping = BTreeMap::new();
            let mut remapped = 0u32;
            let mut dropped = 0u32;

            let mut ingest_rows =
                |key: &str, dest: Option<BundleContextId>| -> Result<(), ContextRemapError> {
                    if let Some(entries) = value.get(key).and_then(NoritoJsonValue::as_array) {
                        for entry in entries {
                            let original = entry
                                .get("original")
                                .and_then(NoritoJsonValue::as_u64)
                                .ok_or(ContextRemapError::Missing("original"))?;
                            let remapped_value = entry
                                .get("remapped")
                                .and_then(NoritoJsonValue::as_u64)
                                .and_then(|raw| u16::try_from(raw).ok())
                                .map(BundleContextId::new)
                                .or(dest);
                            let raw = u16::try_from(original)
                                .map_err(|_| ContextRemapError::InvalidContext(original))?;
                            let target = remapped_value.unwrap_or(BundleContextId::new(raw));
                            mapping.insert(BundleContextId::new(raw), target);
                            if target == escape_context {
                                dropped = dropped.saturating_add(1);
                            } else if target != BundleContextId::new(raw) {
                                remapped = remapped.saturating_add(1);
                            }
                        }
                    }
                    Ok(())
                };

            ingest_rows("kept", None)?;
            ingest_rows("dropped", Some(escape_context))?;
            Ok(Self::new(mapping, escape_context, remapped, dropped))
        }
    }

    /// Load a bundle context remap from a JSON file (output of `streaming-context-remap`).
    pub fn load_bundle_context_remap_from_json<P: AsRef<Path>>(
        path: P,
    ) -> Result<Arc<BundleContextRemap>, ContextRemapError> {
        let text = fs::read_to_string(&path)?;
        let value: NoritoJsonValue = json::from_str(&text)?;
        let remap = BundleContextRemap::from_report(&value)?;
        Ok(Arc::new(remap))
    }

    #[derive(Clone, Debug)]
    pub struct BundleAnsTables {
        precision_bits: u8,
        checksum: Hash,
        max_width: u8,
        tables: [SymbolTable; MAX_BUNDLE_WIDTH],
    }

    impl BundleAnsTables {
        fn uniform(precision_bits: u8) -> Self {
            let tables = core::array::from_fn(|idx| {
                build_uniform_symbol_table((idx + 1) as u8, precision_bits)
            });
            let checksum = hash_bundle_tables(&tables, precision_bits);
            Self {
                precision_bits,
                checksum,
                max_width: MAX_BUNDLE_WIDTH as u8,
                tables,
            }
        }

        fn from_signed(signed: &SignedRansTablesV1) -> Result<Self, BundleTableError> {
            verify_signed_tables(signed)?;
            let mut slots: [Option<SymbolTable>; MAX_BUNDLE_WIDTH] = core::array::from_fn(|_| None);
            let precision_bits = signed
                .payload
                .body
                .groups
                .first()
                .map(|g| g.precision_bits)
                .ok_or(BundleTableError::InvalidStructure(
                    "SignedRansTablesV1 payload must contain at least one group",
                ))?;
            let mut configured_width = signed.payload.body.bundle_width;
            if configured_width == 0 {
                configured_width = MAX_BUNDLE_WIDTH as u8;
            }
            let configured_width = configured_width.max(2).min(MAX_BUNDLE_WIDTH as u8);
            for group in &signed.payload.body.groups {
                let group_size = usize::from(group.group_size);
                if group_size == 0 || !group_size.is_power_of_two() {
                    return Err(BundleTableError::InvalidGroup {
                        bit_len: 0,
                        reason: "group_size must be a power of two",
                    });
                }
                let bit_len = group_size.trailing_zeros() as u8;
                if bit_len == 0 || bit_len as usize > MAX_BUNDLE_WIDTH {
                    return Err(BundleTableError::InvalidGroup {
                        bit_len,
                        reason: "unsupported bundle width",
                    });
                }
                if bit_len > configured_width {
                    return Err(BundleTableError::InvalidGroup {
                        bit_len,
                        reason: "group width exceeds declared bundle width",
                    });
                }
                if group.precision_bits != precision_bits {
                    return Err(BundleTableError::InvalidGroup {
                        bit_len,
                        reason: "precision bits mismatch across groups",
                    });
                }
                let table = SymbolTable::from_group(group)?;
                slots[(bit_len - 1) as usize] = Some(table);
            }
            let mut tables: [SymbolTable; MAX_BUNDLE_WIDTH] = core::array::from_fn(|idx| {
                build_uniform_symbol_table((idx + 1) as u8, precision_bits)
            });
            for bit_len in 2..=configured_width {
                let idx = (bit_len - 1) as usize;
                tables[idx] = slots[idx]
                    .take()
                    .ok_or(BundleTableError::MissingGroup { bit_len })?;
            }
            tables[0] = derive_significance_table(&tables[1]);
            Ok(Self {
                precision_bits,
                checksum: signed.payload.checksum_sha256,
                max_width: configured_width,
                tables,
            })
        }

        fn table_for_bits(&self, bit_len: u8) -> &SymbolTable {
            assert!(
                bit_len <= self.max_width,
                "bundle tables missing width {} (max {})",
                bit_len,
                self.max_width
            );
            let clamped = bit_len.clamp(1, self.max_width);
            &self.tables[clamped as usize - 1]
        }

        pub fn precision_bits(&self) -> u8 {
            self.precision_bits
        }

        pub fn checksum(&self) -> Hash {
            self.checksum
        }

        pub fn max_width(&self) -> u8 {
            self.max_width
        }

        #[cfg(test)]
        pub(crate) fn from_signed_for_tests(
            signed: &SignedRansTablesV1,
        ) -> Result<Self, BundleTableError> {
            Self::from_signed(signed)
        }

        #[cfg(test)]
        pub(crate) fn freq_len_for_bits_for_tests(&self, bit_len: u8) -> Option<usize> {
            if bit_len == 0 || bit_len > self.max_width {
                return None;
            }
            Some(self.table_for_bits(bit_len).freq.len())
        }
    }

    pub fn load_bundle_tables_from_toml<P: AsRef<Path>>(
        path: P,
    ) -> Result<Arc<BundleAnsTables>, BundleTableError> {
        let contents = fs::read_to_string(path)?;
        let toml_value: TomlValue = toml::from_str(&contents)?;
        let json_value = toml_to_norito_value(&toml_value)?;
        let signed: SignedRansTablesV1 = match json::from_value(json_value.clone()) {
            Ok(signed) => signed,
            Err(err) => decode_signed_rans_tables_compat(&json_value)
                .map_err(|_| BundleTableError::Json(err))?,
        };
        let tables = BundleAnsTables::from_signed(&signed)?;
        Ok(Arc::new(tables))
    }

    fn decode_signed_rans_tables_compat(
        raw_value: &NoritoJsonValue,
    ) -> Result<SignedRansTablesV1, BundleTableError> {
        let mut normalized = raw_value.clone();
        expand_jsonish_strings(&mut normalized);
        coerce_jsonish_scalar_strings(&mut normalized);
        if let Ok(signed) = json::from_value(normalized.clone()) {
            return Ok(signed);
        }
        decode_signed_rans_tables_manual(&normalized)
    }

    fn parse_jsonish_norito_value(raw: &str) -> Result<NoritoJsonValue, json::Error> {
        let trimmed = raw.trim();
        if let Ok(value) = json::parse_value(trimmed) {
            return Ok(value);
        }

        if let Ok(inner) = json::from_str::<String>(trimmed) {
            let inner_trimmed = inner.trim();
            if let Ok(value) = json::parse_value(inner_trimmed) {
                return Ok(value);
            }
            let normalized_inner = normalize_jsonish_payload(inner_trimmed);
            if let Ok(value) = json::parse_value(&normalized_inner) {
                return Ok(value);
            }
        }

        let normalized = normalize_jsonish_payload(trimmed);
        json::parse_value(&normalized)
    }

    fn normalize_jsonish_payload(raw: &str) -> String {
        let mut out = raw.trim().trim_matches(&['"', '\''][..]).to_owned();
        if out.contains("\\\"") {
            out = out.replace("\\\"", "\"");
        }
        if out.contains("\\{") || out.contains("\\}") || out.contains("\\[") || out.contains("\\]")
        {
            out = out
                .replace("\\{", "{")
                .replace("\\}", "}")
                .replace("\\[", "[")
                .replace("\\]", "]");
        }

        out = replace_equals_and_missing_colons(&out);
        strip_trailing_commas(&out)
    }

    fn replace_equals_and_missing_colons(input: &str) -> String {
        let chars: Vec<char> = input.chars().collect();
        let mut out = String::with_capacity(input.len() + 16);
        let mut i = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        let mut string_is_key = false;
        let mut after_object_key = false;
        let mut context_stack: Vec<char> = Vec::new();
        let mut object_expect_key = false;

        while i < chars.len() {
            let ch = chars[i];

            if !in_string && after_object_key && !ch.is_whitespace() {
                if ch == ':' || ch == '=' {
                    out.push(':');
                    after_object_key = false;
                    object_expect_key = false;
                    i += 1;
                    continue;
                }
                out.push(':');
                after_object_key = false;
                object_expect_key = false;
                continue;
            }

            if in_string {
                out.push(ch);
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                    if string_is_key {
                        after_object_key = true;
                    }
                }
                i += 1;
                continue;
            }

            match ch {
                '"' => {
                    let in_object = context_stack.last() == Some(&'{');
                    string_is_key = in_object && object_expect_key;
                    in_string = true;
                    escaped = false;
                    out.push(ch);
                }
                '{' => {
                    context_stack.push('{');
                    object_expect_key = true;
                    after_object_key = false;
                    out.push(ch);
                }
                '[' => {
                    context_stack.push('[');
                    object_expect_key = false;
                    after_object_key = false;
                    out.push(ch);
                }
                '}' => {
                    if context_stack.pop() == Some('{') {
                        object_expect_key = false;
                    }
                    after_object_key = false;
                    out.push(ch);
                }
                ']' => {
                    context_stack.pop();
                    after_object_key = false;
                    out.push(ch);
                }
                ',' => {
                    out.push(ch);
                    after_object_key = false;
                    object_expect_key = context_stack.last() == Some(&'{');
                }
                ':' => {
                    out.push(ch);
                    after_object_key = false;
                    object_expect_key = false;
                }
                '=' => {
                    out.push(':');
                    after_object_key = false;
                    object_expect_key = false;
                }
                _ => out.push(ch),
            }

            i += 1;
        }

        out
    }

    fn strip_trailing_commas(input: &str) -> String {
        let chars: Vec<char> = input.chars().collect();
        let mut out = String::with_capacity(input.len());
        let mut in_string = false;
        let mut escaped = false;
        let mut i = 0usize;

        while i < chars.len() {
            let ch = chars[i];
            if in_string {
                out.push(ch);
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                i += 1;
                continue;
            }

            if ch == '"' {
                in_string = true;
                escaped = false;
                out.push(ch);
                i += 1;
                continue;
            }

            if ch == ',' {
                let mut lookahead = i + 1;
                while lookahead < chars.len() && chars[lookahead].is_whitespace() {
                    lookahead += 1;
                }
                if lookahead < chars.len() && (chars[lookahead] == '}' || chars[lookahead] == ']')
                {
                    i += 1;
                    continue;
                }
            }

            out.push(ch);
            i += 1;
        }

        out
    }

    fn expand_jsonish_strings(value: &mut NoritoJsonValue) {
        match value {
            NoritoJsonValue::String(raw) => {
                if let Ok(mut parsed) = parse_jsonish_norito_value(raw) {
                    expand_jsonish_strings(&mut parsed);
                    *value = parsed;
                }
            }
            NoritoJsonValue::Array(items) => {
                for item in items {
                    expand_jsonish_strings(item);
                }
            }
            NoritoJsonValue::Object(map) => {
                for nested in map.values_mut() {
                    expand_jsonish_strings(nested);
                }
            }
            _ => {}
        }
    }

    fn coerce_jsonish_scalar_strings(value: &mut NoritoJsonValue) {
        match value {
            NoritoJsonValue::String(raw) => {
                let trimmed = raw.trim();
                if trimmed.eq_ignore_ascii_case("true") {
                    *value = NoritoJsonValue::Bool(true);
                } else if trimmed.eq_ignore_ascii_case("false") {
                    *value = NoritoJsonValue::Bool(false);
                } else if trimmed.eq_ignore_ascii_case("null") {
                    *value = NoritoJsonValue::Null;
                } else if let Ok(parsed) = trimmed.parse::<u64>() {
                    *value = NoritoJsonValue::Number(json::native::Number::U64(parsed));
                } else if let Ok(parsed) = trimmed.parse::<i64>() {
                    *value = NoritoJsonValue::Number(json::native::Number::I64(parsed));
                }
            }
            NoritoJsonValue::Array(items) => {
                for item in items {
                    coerce_jsonish_scalar_strings(item);
                }
            }
            NoritoJsonValue::Object(map) => {
                for nested in map.values_mut() {
                    coerce_jsonish_scalar_strings(nested);
                }
            }
            _ => {}
        }
    }

    fn decode_signed_rans_tables_manual(
        raw_value: &NoritoJsonValue,
    ) -> Result<SignedRansTablesV1, BundleTableError> {
        if let Some(raw) = raw_value.as_str() {
            let mut parsed = parse_jsonish_norito_value(raw).map_err(BundleTableError::Json)?;
            expand_jsonish_strings(&mut parsed);
            coerce_jsonish_scalar_strings(&mut parsed);
            return decode_signed_rans_tables_manual(&parsed);
        }

        let root = raw_value
            .as_object()
            .ok_or(BundleTableError::InvalidStructure(
                "SignedRansTablesV1 root must be an object",
            ))?;

        let (payload_value, signature_value) = if let Some(payload) = root.get("payload") {
            (payload, root.get("signature"))
        } else {
            // Compatibility: some legacy payloads persisted only the payload body.
            (raw_value, None)
        };

        let payload = parse_rans_tables_payload(payload_value)?;
        let signature = match signature_value {
            Some(value) => parse_rans_tables_signature(value)?,
            None => None,
        };
        Ok(SignedRansTablesV1 { payload, signature })
    }

    fn parse_rans_tables_payload(value: &NoritoJsonValue) -> Result<RansTablesV1, BundleTableError> {
        if let Some(raw) = value.as_str() {
            let mut parsed = parse_jsonish_norito_value(raw).map_err(BundleTableError::Json)?;
            expand_jsonish_strings(&mut parsed);
            coerce_jsonish_scalar_strings(&mut parsed);
            return parse_rans_tables_payload(&parsed);
        }

        let payload = value
            .as_object()
            .ok_or(BundleTableError::InvalidStructure(
                "SignedRansTablesV1 payload must be an object",
            ))?;

        let body_value = payload
            .get("body")
            .ok_or(BundleTableError::InvalidStructure(
                "SignedRansTablesV1 payload missing body",
            ))?;
        let body = parse_rans_tables_body(body_value)?;

        Ok(RansTablesV1 {
            version: parse_u16_field(
                payload,
                "version",
                "SignedRansTablesV1 payload version must be an integer",
            )?,
            generated_at: parse_u64_field(
                payload,
                "generated_at",
                "SignedRansTablesV1 payload generated_at must be an integer",
            )?,
            generator_commit: parse_string_field(
                payload,
                "generator_commit",
                "SignedRansTablesV1 payload generator_commit must be a string",
            )?,
            checksum_sha256: parse_hex_field::<32>(
                payload,
                "checksum_sha256",
                "SignedRansTablesV1 payload checksum_sha256 must be a 64-character hex string",
            )?,
            body,
        })
    }

    fn parse_rans_tables_body(value: &NoritoJsonValue) -> Result<RansTablesBodyV1, BundleTableError> {
        if let Some(raw) = value.as_str() {
            let mut parsed = parse_jsonish_norito_value(raw).map_err(BundleTableError::Json)?;
            expand_jsonish_strings(&mut parsed);
            coerce_jsonish_scalar_strings(&mut parsed);
            return parse_rans_tables_body(&parsed);
        }

        let body = value
            .as_object()
            .ok_or(BundleTableError::InvalidStructure(
                "SignedRansTablesV1 payload body must be an object",
            ))?;

        let groups_value = body
            .get("groups")
            .ok_or(BundleTableError::InvalidStructure(
                "SignedRansTablesV1 payload body missing groups",
            ))?;
        let groups_array = groups_value
            .as_array()
            .ok_or(BundleTableError::InvalidStructure(
                "SignedRansTablesV1 payload body groups must be an array",
            ))?;
        let mut groups = Vec::with_capacity(groups_array.len());
        for group in groups_array {
            groups.push(parse_rans_group(group)?);
        }

        Ok(RansTablesBodyV1 {
            seed: parse_u64_field(
                body,
                "seed",
                "SignedRansTablesV1 payload body seed must be an integer",
            )?,
            bundle_width: parse_u8_field(
                body,
                "bundle_width",
                "SignedRansTablesV1 payload body bundle_width must be an integer",
            )?,
            groups,
        })
    }

    fn parse_rans_group(value: &NoritoJsonValue) -> Result<RansGroupTableV1, BundleTableError> {
        if let Some(raw) = value.as_str() {
            let mut parsed = parse_jsonish_norito_value(raw).map_err(BundleTableError::Json)?;
            expand_jsonish_strings(&mut parsed);
            coerce_jsonish_scalar_strings(&mut parsed);
            return parse_rans_group(&parsed);
        }

        let group = value
            .as_object()
            .ok_or(BundleTableError::InvalidStructure(
                "SignedRansTablesV1 payload group must be an object",
            ))?;

        let frequencies = parse_u16_array_field(
            group,
            "frequencies",
            "SignedRansTablesV1 payload group frequencies must be an integer array",
        )?;
        let cumulative = parse_u32_array_field(
            group,
            "cumulative",
            "SignedRansTablesV1 payload group cumulative must be an integer array",
        )?;

        Ok(RansGroupTableV1 {
            width_bits: parse_u8_field(
                group,
                "width_bits",
                "SignedRansTablesV1 payload group width_bits must be an integer",
            )?,
            group_size: parse_u16_field(
                group,
                "group_size",
                "SignedRansTablesV1 payload group group_size must be an integer",
            )?,
            precision_bits: parse_u8_field(
                group,
                "precision_bits",
                "SignedRansTablesV1 payload group precision_bits must be an integer",
            )?,
            frequencies,
            cumulative,
        })
    }

    fn parse_rans_tables_signature(
        value: &NoritoJsonValue,
    ) -> Result<Option<RansTablesSignatureV1>, BundleTableError> {
        if matches!(value, NoritoJsonValue::Null) {
            return Ok(None);
        }
        if let Some(raw) = value.as_str() {
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
                return Ok(None);
            }
            let mut parsed = parse_jsonish_norito_value(raw).map_err(BundleTableError::Json)?;
            expand_jsonish_strings(&mut parsed);
            coerce_jsonish_scalar_strings(&mut parsed);
            return parse_rans_tables_signature(&parsed);
        }

        let signature = value
            .as_object()
            .ok_or(BundleTableError::InvalidStructure(
                "SignedRansTablesV1 signature must be an object",
            ))?;
        let algorithm = parse_string_field(
            signature,
            "algorithm",
            "SignedRansTablesV1 signature algorithm must be a string",
        )?;
        let algorithm = match algorithm.trim().to_ascii_lowercase().as_str() {
            "ed25519" => SignatureAlgorithm::Ed25519,
            _ => {
                return Err(BundleTableError::InvalidStructure(
                    "SignedRansTablesV1 signature algorithm must be `ed25519`",
                ))
            }
        };

        Ok(Some(RansTablesSignatureV1 {
            algorithm,
            public_key: parse_hex_field::<32>(
                signature,
                "public_key",
                "SignedRansTablesV1 signature public_key must be a 64-character hex string",
            )?,
            signature: parse_hex_field::<64>(
                signature,
                "signature",
                "SignedRansTablesV1 signature signature must be a 128-character hex string",
            )?,
        }))
    }

    fn parse_string_field(
        map: &BTreeMap<String, NoritoJsonValue>,
        key: &'static str,
        err: &'static str,
    ) -> Result<String, BundleTableError> {
        let value = map
            .get(key)
            .ok_or(BundleTableError::InvalidStructure(err))?;
        value
            .as_str()
            .map(str::to_owned)
            .ok_or(BundleTableError::InvalidStructure(err))
    }

    fn parse_u64_field(
        map: &BTreeMap<String, NoritoJsonValue>,
        key: &'static str,
        err: &'static str,
    ) -> Result<u64, BundleTableError> {
        let value = map
            .get(key)
            .ok_or(BundleTableError::InvalidStructure(err))?;
        parse_u64_value(value, err)
    }

    fn parse_u16_field(
        map: &BTreeMap<String, NoritoJsonValue>,
        key: &'static str,
        err: &'static str,
    ) -> Result<u16, BundleTableError> {
        let value = map
            .get(key)
            .ok_or(BundleTableError::InvalidStructure(err))?;
        let parsed = parse_u64_value(value, err)?;
        u16::try_from(parsed).map_err(|_| BundleTableError::InvalidStructure(err))
    }

    fn parse_u8_field(
        map: &BTreeMap<String, NoritoJsonValue>,
        key: &'static str,
        err: &'static str,
    ) -> Result<u8, BundleTableError> {
        let value = map
            .get(key)
            .ok_or(BundleTableError::InvalidStructure(err))?;
        let parsed = parse_u64_value(value, err)?;
        u8::try_from(parsed).map_err(|_| BundleTableError::InvalidStructure(err))
    }

    fn parse_u64_value(value: &NoritoJsonValue, err: &'static str) -> Result<u64, BundleTableError> {
        if let Some(parsed) = value.as_u64() {
            return Ok(parsed);
        }
        if let Some(parsed) = value.as_i64()
            && parsed >= 0
        {
            return Ok(parsed as u64);
        }
        if let Some(raw) = value.as_str()
            && let Ok(parsed) = raw.trim().parse::<u64>()
        {
            return Ok(parsed);
        }
        Err(BundleTableError::InvalidStructure(err))
    }

    fn parse_u16_array_field(
        map: &BTreeMap<String, NoritoJsonValue>,
        key: &'static str,
        err: &'static str,
    ) -> Result<Vec<u16>, BundleTableError> {
        let value = map
            .get(key)
            .ok_or(BundleTableError::InvalidStructure(err))?;
        let array = value
            .as_array()
            .ok_or(BundleTableError::InvalidStructure(err))?;
        let mut out = Vec::with_capacity(array.len());
        for entry in array {
            let parsed = parse_u64_value(entry, err)?;
            out.push(u16::try_from(parsed).map_err(|_| BundleTableError::InvalidStructure(err))?);
        }
        Ok(out)
    }

    fn parse_u32_array_field(
        map: &BTreeMap<String, NoritoJsonValue>,
        key: &'static str,
        err: &'static str,
    ) -> Result<Vec<u32>, BundleTableError> {
        let value = map
            .get(key)
            .ok_or(BundleTableError::InvalidStructure(err))?;
        let array = value
            .as_array()
            .ok_or(BundleTableError::InvalidStructure(err))?;
        let mut out = Vec::with_capacity(array.len());
        for entry in array {
            out.push(u32::try_from(parse_u64_value(entry, err)?).map_err(|_| {
                BundleTableError::InvalidStructure(err)
            })?);
        }
        Ok(out)
    }

    fn parse_hex_field<const N: usize>(
        map: &BTreeMap<String, NoritoJsonValue>,
        key: &'static str,
        err: &'static str,
    ) -> Result<[u8; N], BundleTableError> {
        let value = map
            .get(key)
            .ok_or(BundleTableError::InvalidStructure(err))?;
        let raw = value.as_str().ok_or(BundleTableError::InvalidStructure(err))?;
        parse_hex_array::<N>(raw, err)
    }

    fn parse_hex_array<const N: usize>(
        raw: &str,
        err: &'static str,
    ) -> Result<[u8; N], BundleTableError> {
        let trimmed = raw.trim();
        if trimmed.len() != N * 2 {
            return Err(BundleTableError::InvalidStructure(err));
        }
        let bytes = trimmed.as_bytes();
        let mut out = [0u8; N];
        for idx in 0..N {
            let hi = hex_nibble(bytes[idx * 2]).ok_or(BundleTableError::InvalidStructure(err))?;
            let lo =
                hex_nibble(bytes[idx * 2 + 1]).ok_or(BundleTableError::InvalidStructure(err))?;
            out[idx] = (hi << 4) | lo;
        }
        Ok(out)
    }

    fn hex_nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    /// Lazily construct the shared default bundle tables (uniform distribution).
    ///
    /// The returned handle is backed by a `OnceLock`, so multiple callers share the
    /// same allocation and precision parameters.
    pub fn default_bundle_tables() -> Arc<BundleAnsTables> {
        static DEFAULT: OnceLock<Arc<BundleAnsTables>> = OnceLock::new();
        DEFAULT
            .get_or_init(|| Arc::new(BundleAnsTables::uniform(BUNDLE_ANS_PRECISION_BITS)))
            .clone()
    }

    fn hash_bundle_tables(tables: &[SymbolTable; MAX_BUNDLE_WIDTH], precision_bits: u8) -> Hash {
        let mut hasher = Sha256::new();
        hasher.update([precision_bits]);
        for table in tables {
            hasher.update((table.freq.len() as u32).to_le_bytes());
            for &freq in &table.freq {
                hasher.update(freq.to_le_bytes());
            }
        }
        let digest = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&digest);
        hash
    }

    fn verify_signed_tables(signed: &SignedRansTablesV1) -> Result<(), BundleTableError> {
        let bytes = norito_core::to_bytes(&signed.payload.body)
            .map_err(|_| BundleTableError::InvalidStructure("failed to encode table body"))?;
        let digest = Sha256::digest(bytes);
        let mut checksum = [0u8; 32];
        checksum.copy_from_slice(digest.as_ref());
        if checksum != signed.payload.checksum_sha256 {
            return Err(BundleTableError::ChecksumMismatch);
        }
        Ok(())
    }

    fn toml_to_norito_value(value: &TomlValue) -> Result<NoritoJsonValue, BundleTableError> {
        use toml::Value::{Array, Boolean, Datetime, Float, Integer, String as TomlString, Table};
        Ok(match value {
            Boolean(b) => NoritoJsonValue::Bool(*b),
            Integer(i) => {
                if *i >= 0 {
                    NoritoJsonValue::Number(json::native::Number::U64(*i as u64))
                } else {
                    NoritoJsonValue::Number(json::native::Number::I64(*i))
                }
            }
            Float(f) => NoritoJsonValue::Number(json::native::Number::F64(*f)),
            TomlString(s) => NoritoJsonValue::String(s.clone()),
            Datetime(dt) => NoritoJsonValue::String(dt.to_string()),
            Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(toml_to_norito_value(item)?);
                }
                NoritoJsonValue::Array(out)
            }
            Table(map) => {
                let mut out = BTreeMap::new();
                for (k, v) in map {
                    out.insert(k.clone(), toml_to_norito_value(v)?);
                }
                NoritoJsonValue::Object(out)
            }
        })
    }

    const BASELINE_QUANT_MATRIX: [u8; 64] = [
        16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69,
        56, 14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81,
        104, 113, 92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
    ];

    const ZIG_ZAG: [usize; 64] = [
        0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27,
        20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51,
        58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
    ];

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub(crate) enum FrameType {
        Intra,
        Predicted,
    }

    impl FrameType {
        fn as_byte(self) -> u8 {
            match self {
                Self::Intra => 0,
                Self::Predicted => 1,
            }
        }

        pub(crate) fn from_byte(byte: u8) -> Result<Self, CodecError> {
            match byte {
                0 => Ok(Self::Intra),
                1 => Ok(Self::Predicted),
                other => Err(CodecError::UnknownFrameType(other)),
            }
        }

        pub(crate) fn reset_dc(self) -> bool {
            matches!(self, FrameType::Intra)
        }
    }

    /// Segment encoding parameters for the baseline profile.
    #[derive(Clone, Debug)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct BaselineEncoderConfig {
        pub profile: ProfileId,
        pub encryption_suite: EncryptionSuite,
        pub layer_bitmap: u32,
        pub duration_ns: u32,
        pub storage_class: StorageClass,
        pub feedback_hint: FeedbackHint,
        pub frame_dimensions: FrameDimensions,
        pub frames_per_segment: u16,
        pub frame_duration_ns: u32,
        pub quantizer: u8,
        pub audio: Option<AudioEncoderConfig>,
        pub entropy_mode: EntropyMode,
        pub bundle_width: u8,
        pub bundle_tables: Arc<BundleAnsTables>,
        pub bundle_acceleration: BundleAcceleration,
        pub bundle_context_remap: Option<Arc<BundleContextRemap>>,
        /// Optional prefetch distance (in records) for the bundle ANS encoder. A value
        /// of `0` disables prefetching.
        pub bundle_prefetch_distance: u16,
        pub rdo_mode: RdoMode,
        pub rdo_neural_seed: Option<Hash>,
    }

    impl Default for BaselineEncoderConfig {
        fn default() -> Self {
            Self {
                profile: ProfileId::BASELINE,
                encryption_suite: EncryptionSuite::X25519ChaCha20Poly1305([0u8; 32]),
                layer_bitmap: 0b1,
                duration_ns: 250_000_000,
                storage_class: StorageClass::Ephemeral,
                feedback_hint: FeedbackHint::default(),
                frame_dimensions: FrameDimensions::new(640, 360),
                frames_per_segment: 1,
                frame_duration_ns: 33_333_333,
                quantizer: 16,
                audio: None,
                entropy_mode: EntropyMode::RansBundled,
                bundle_width: 2,
                bundle_tables: default_bundle_tables(),
                bundle_acceleration: BundleAcceleration::None,
                bundle_context_remap: None,
                bundle_prefetch_distance: 0,
                rdo_mode: RdoMode::None,
                rdo_neural_seed: None,
            }
        }
    }

    impl BaselineEncoderConfig {
        #[must_use]
        pub fn with_audio(mut self, audio: AudioEncoderConfig) -> Self {
            self.audio = Some(audio);
            self
        }

        #[must_use]
        pub fn with_bundle_tables(mut self, tables: Arc<BundleAnsTables>) -> Self {
            self.bundle_tables = tables;
            self
        }

        #[must_use]
        pub fn with_bundle_acceleration(mut self, accel: BundleAcceleration) -> Self {
            self.bundle_acceleration = accel;
            self
        }

        #[must_use]
        pub fn with_bundle_context_remap(mut self, remap: Arc<BundleContextRemap>) -> Self {
            self.bundle_context_remap = Some(remap);
            self
        }

        #[must_use]
        pub fn with_bundle_prefetch_distance(mut self, distance: u16) -> Self {
            self.bundle_prefetch_distance = distance;
            self
        }

        #[must_use]
        pub fn with_rdo_mode(mut self, mode: RdoMode) -> Self {
            self.rdo_mode = mode;
            self
        }

        #[must_use]
        pub fn with_rdo_neural_seed(mut self, seed: Hash) -> Self {
            self.rdo_neural_seed = Some(seed);
            self
        }
    }

    /// Parameters required to construct a manifest for a baseline segment.
    #[derive(Clone, Debug)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct BaselineManifestParams {
        pub stream_id: Hash,
        pub protocol_version: u16,
        pub published_at: Timestamp,
        pub da_endpoint: Multiaddr,
        pub privacy_routes: Vec<PrivacyRoute>,
        pub public_metadata: StreamMetadata,
        pub capabilities: CapabilityFlags,
        pub signature: Signature,
        pub fec_suite: FecScheme,
        pub neural_bundle: Option<NeuralBundle>,
        pub transport_capabilities_hash: Hash,
    }

    impl Default for BaselineManifestParams {
        fn default() -> Self {
            Self {
                stream_id: [0u8; 32],
                protocol_version: 1,
                published_at: 0,
                da_endpoint: Multiaddr::default(),
                privacy_routes: Vec::new(),
                public_metadata: StreamMetadata::default(),
                capabilities: CapabilityFlags::default(),
                signature: [0u8; 64],
                fec_suite: FecScheme::Rs12_10,
                neural_bundle: None,
                transport_capabilities_hash: [0u8; 32],
            }
        }
    }

    /// Encoded segment artifacts produced by the baseline encoder.
    #[derive(Clone, Debug)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct EncodedSegment {
        pub header: SegmentHeader,
        pub descriptors: Vec<ChunkDescriptor>,
        pub chunks: Vec<Vec<u8>>,
        pub audio: Option<SegmentAudio>,
    }

    impl EncodedSegment {
        /// Construct a manifest that matches the encoded segment.
        pub fn build_manifest(&self, mut params: BaselineManifestParams) -> ManifestV1 {
            assert!(
                self.header.entropy_mode.is_bundled(),
                "rANS manifests without bundled tables are not supported"
            );
            params.capabilities = params
                .capabilities
                .insert(CapabilityFlags::FEATURE_ENTROPY_BUNDLED);
            let accel_mask = CapabilityFlags::FEATURE_BUNDLE_ACCEL_CPU_SIMD
                | CapabilityFlags::FEATURE_BUNDLE_ACCEL_GPU;
            params.capabilities = params.capabilities.remove(accel_mask);
            let required = self.header.bundle_acceleration.capability_mask();
            if required != 0 {
                params.capabilities = params.capabilities.insert(required);
            }
            ManifestV1 {
                stream_id: params.stream_id,
                protocol_version: params.protocol_version,
                segment_number: self.header.segment_number,
                published_at: params.published_at,
                profile: self.header.profile,
                entropy_mode: self.header.entropy_mode,
                entropy_tables_checksum: self.header.entropy_tables_checksum,
                da_endpoint: params.da_endpoint,
                chunk_root: self.header.chunk_merkle_root,
                content_key_id: self.header.content_key_id,
                nonce_salt: self.header.nonce_salt,
                chunk_descriptors: self.descriptors.clone(),
                transport_capabilities_hash: params.transport_capabilities_hash,
                encryption_suite: self.header.encryption_suite,
                fec_suite: params.fec_suite,
                privacy_routes: params.privacy_routes,
                neural_bundle: params.neural_bundle,
                audio_summary: self.header.audio_summary,
                public_metadata: params.public_metadata,
                capabilities: params.capabilities,
                signature: params.signature,
            }
        }

        /// Verify that a manifest is consistent with the encoded segment.
        pub fn verify_manifest(&self, manifest: &ManifestV1) -> Result<(), ManifestError> {
            verify_segment(
                &self.header,
                &self.descriptors,
                &self.chunks,
                self.audio.as_ref(),
            )
            .map_err(ManifestError::from)?;

            if manifest.segment_number != self.header.segment_number {
                return Err(ManifestError::SegmentNumberMismatch);
            }
            if manifest.profile != self.header.profile {
                return Err(ManifestError::ProfileMismatch);
            }
            if manifest.entropy_mode != self.header.entropy_mode {
                return Err(ManifestError::EntropyModeMismatch);
            }
            if manifest.entropy_mode.is_bundled()
                && manifest.entropy_tables_checksum != self.header.entropy_tables_checksum
            {
                return Err(ManifestError::EntropyTablesMismatch);
            }
            if manifest.encryption_suite != self.header.encryption_suite {
                return Err(ManifestError::EncryptionSuiteMismatch);
            }
            if manifest.content_key_id != self.header.content_key_id {
                return Err(ManifestError::ContentKeyIdMismatch);
            }
            if manifest.nonce_salt != self.header.nonce_salt {
                return Err(ManifestError::NonceSaltMismatch);
            }
            if manifest.chunk_root != self.header.chunk_merkle_root {
                return Err(ManifestError::ChunkRootMismatch);
            }
            if manifest.audio_summary != self.header.audio_summary {
                return Err(ManifestError::AudioSummaryMismatch);
            }
            if manifest.chunk_descriptors.len() != self.descriptors.len() {
                return Err(ManifestError::DescriptorCountMismatch);
            }
            for (idx, (expected, actual)) in self
                .descriptors
                .iter()
                .zip(manifest.chunk_descriptors.iter())
                .enumerate()
            {
                if expected != actual {
                    return Err(ManifestError::DescriptorMismatch(saturating_usize_to_u32(
                        idx,
                    )));
                }
            }
            let required_bundled = self.header.entropy_mode.is_bundled();
            let found_bundled = manifest
                .capabilities
                .contains(CapabilityFlags::FEATURE_ENTROPY_BUNDLED);
            if required_bundled != found_bundled {
                return Err(ManifestError::CapabilityEntropyFlagMismatch {
                    required_bundled,
                    found_bundled,
                });
            }
            let accel_mask = CapabilityFlags::FEATURE_BUNDLE_ACCEL_CPU_SIMD
                | CapabilityFlags::FEATURE_BUNDLE_ACCEL_GPU;
            let advertised_mask = manifest.capabilities.bits() & accel_mask;
            let required_mask = if required_bundled {
                self.header.bundle_acceleration.capability_mask()
            } else {
                0
            };
            if advertised_mask != required_mask {
                return Err(ManifestError::CapabilityAccelerationFlagMismatch {
                    entropy_mode: manifest.entropy_mode,
                    required_mask,
                    found_mask: advertised_mask,
                });
            }
            Ok(())
        }

        /// Wrap the encoded segment into a portable bundle for RD/decoder tooling.
        pub fn to_bundle(
            &self,
            frame_dimensions: FrameDimensions,
            frame_duration_ns: u32,
        ) -> SegmentBundle {
            self.to_bundle_with_chroma(frame_dimensions, frame_duration_ns, Vec::new())
        }

        /// Wrap the encoded segment into a portable bundle for RD/decoder tooling, attaching optional chroma sidecars.
        pub fn to_bundle_with_chroma(
            &self,
            frame_dimensions: FrameDimensions,
            frame_duration_ns: u32,
            chroma: Vec<Chroma420Frame>,
        ) -> SegmentBundle {
            SegmentBundle {
                header: self.header.clone(),
                descriptors: self.descriptors.clone(),
                chunks: self.chunks.clone(),
                audio: self.audio.clone(),
                frame_dimensions,
                frame_duration_ns,
                chroma,
            }
        }
    }

    /// Serialized segment bundle carrying the encoded payloads and decoder metadata.
    #[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct SegmentBundle {
        /// Segment header as persisted alongside chunk data.
        pub header: SegmentHeader,
        /// Chunk descriptors matching the payload ordering.
        pub descriptors: Vec<ChunkDescriptor>,
        /// Raw chunk payloads.
        pub chunks: Vec<Bytes>,
        /// Optional encoded audio track.
        #[norito(skip_serializing_if = "Option::is_none")]
        #[norito(default)]
        pub audio: Option<SegmentAudio>,
        /// Frame geometry required for decode and Y4M reconstruction.
        pub frame_dimensions: FrameDimensions,
        /// Frame duration in nanoseconds.
        pub frame_duration_ns: u32,
        /// Optional 4:2:0 chroma planes matching the decoded frames.
        #[norito(default)]
        #[norito(skip_serializing_if = "Vec::is_empty")]
        pub chroma: Vec<Chroma420Frame>,
    }

    impl SegmentBundle {
        /// Convert the bundle back into an encoded segment, validating chunk commitments.
        pub fn into_segment(self) -> Result<(EncodedSegment, FrameDimensions, u32), SegmentError> {
            let (segment, dims, duration, _) = self.into_segment_with_chroma()?;
            Ok((segment, dims, duration))
        }

        /// Convert the bundle back into an encoded segment and return the attached chroma frames.
        pub fn into_segment_with_chroma(
            self,
        ) -> Result<(EncodedSegment, FrameDimensions, u32, Vec<Chroma420Frame>), SegmentError>
        {
            let chroma = self.chroma;
            let segment = EncodedSegment {
                header: self.header,
                descriptors: self.descriptors,
                chunks: self.chunks,
                audio: self.audio,
            };
            verify_segment(
                &segment.header,
                &segment.descriptors,
                &segment.chunks,
                segment.audio.as_ref(),
            )?;
            Ok((
                segment,
                self.frame_dimensions,
                self.frame_duration_ns,
                chroma,
            ))
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct ChunkListCountMismatch {
        pub descriptors: u32,
        pub chunks: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct HeaderDescriptorCountMismatch {
        pub header: u16,
        pub actual: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct ChunkCountOverflowInfo {
        pub found: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct DescriptorOffsetDetails {
        pub index: u32,
        pub expected: u32,
        pub actual: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct DescriptorLengthDetails {
        pub index: u32,
        pub descriptor: u32,
        pub chunk: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct AudioSampleRateMismatchInfo {
        pub expected: u32,
        pub found: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct AudioFrameSamplesMismatchInfo {
        pub expected: u16,
        pub found: u16,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct AudioFrameDurationMismatchInfo {
        pub expected: u32,
        pub found: u32,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct AudioFrameCountMismatchInfo {
        pub expected: u16,
        pub found: u16,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct AudioFecMismatchInfo {
        pub expected: u8,
        pub found: u8,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct AudioLayoutMismatchInfo {
        pub expected: AudioLayout,
        pub found: AudioLayout,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct AudioTimestampMismatchInfo {
        pub index: u32,
        pub expected: u64,
        pub found: u64,
    }

    /// Errors emitted when verifying or decoding NSC segments.
    #[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub enum SegmentError {
        #[error(transparent)]
        Chunk(#[from] ChunkError),
        #[error(
            "chunk descriptor count ({descriptors}) does not match chunk list ({chunks})",
            descriptors = .0.descriptors,
            chunks = .0.chunks
        )]
        CountMismatch(ChunkListCountMismatch),
        #[error("chunk count exceeds u16 range: found {found}", found = .0.found)]
        ChunkCountOverflow(ChunkCountOverflowInfo),
        #[error(
            "header chunk count ({header}) does not match descriptor list ({actual})",
            header = .0.header,
            actual = .0.actual
        )]
        HeaderCountMismatch(HeaderDescriptorCountMismatch),
        #[error("chunk commitment mismatch at index {0}")]
        CommitmentMismatch(u32),
        #[error(
            "descriptor offset mismatch at index {index}: expected {expected}, found {actual}",
            index = .0.index,
            expected = .0.expected,
            actual = .0.actual
        )]
        DescriptorOffsetMismatch(DescriptorOffsetDetails),
        #[error(
            "descriptor length mismatch at index {index}: descriptor {descriptor}, chunk {chunk}",
            index = .0.index,
            descriptor = .0.descriptor,
            chunk = .0.chunk
        )]
        DescriptorLengthMismatch(DescriptorLengthDetails),
        #[error("chunk length at index {0} exceeds u32 range")]
        ChunkLengthOverflow(u32),
        #[error("descriptor offsets overflow while accumulating at index {0}")]
        OffsetOverflow(u32),
        #[error("merkle root mismatch")]
        MerkleMismatch,
        #[error("chunk ids must be strictly ascending")]
        UnsortedChunkIds,
        #[error("segment header expects audio track but none was provided")]
        AudioSummaryMissing,
        #[error("segment carried unexpected audio track not advertised by header")]
        AudioSummaryUnexpected,
        #[error("audio sample rate mismatch: expected {expected}, found {found}", expected = .0.expected, found = .0.found)]
        AudioSampleRateMismatch(AudioSampleRateMismatchInfo),
        #[error("audio frame sample count mismatch: expected {expected}, found {found}", expected = .0.expected, found = .0.found)]
        AudioFrameSamplesMismatch(AudioFrameSamplesMismatchInfo),
        #[error("audio frame duration mismatch: expected {expected}, found {found}", expected = .0.expected, found = .0.found)]
        AudioFrameDurationMismatch(AudioFrameDurationMismatchInfo),
        #[error("audio frame count mismatch: expected {expected}, found {found}", expected = .0.expected, found = .0.found)]
        AudioFrameCountMismatch(AudioFrameCountMismatchInfo),
        #[error("audio FEC level mismatch: expected {expected}, found {found}", expected = .0.expected, found = .0.found)]
        AudioFecMismatch(AudioFecMismatchInfo),
        #[error("audio layout mismatch: expected {expected:?}, found {found:?}", expected = .0.expected, found = .0.found)]
        AudioLayoutMismatch(AudioLayoutMismatchInfo),
        #[error("audio timestamp mismatch at index {index}: expected {expected}, found {found}", index = .0.index, expected = .0.expected, found = .0.found)]
        AudioTimestampMismatch(AudioTimestampMismatchInfo),
        #[error("audio timestamp computation overflow at index {0}")]
        AudioTimestampOverflow(u32),
    }

    /// Errors emitted when validating a manifest against an encoded segment.
    #[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub enum ManifestError {
        #[error(transparent)]
        Segment(#[from] SegmentError),
        #[error("segment number mismatch")]
        SegmentNumberMismatch,
        #[error("profile mismatch")]
        ProfileMismatch,
        #[error("entropy mode mismatch")]
        EntropyModeMismatch,
        #[error("entropy tables checksum mismatch")]
        EntropyTablesMismatch,
        #[error("encryption suite mismatch")]
        EncryptionSuiteMismatch,
        #[error("content key id mismatch")]
        ContentKeyIdMismatch,
        #[error("nonce salt mismatch")]
        NonceSaltMismatch,
        #[error("chunk root mismatch")]
        ChunkRootMismatch,
        #[error("audio summary mismatch between manifest and segment")]
        AudioSummaryMismatch,
        #[error("descriptor count mismatch")]
        DescriptorCountMismatch,
        #[error("descriptor mismatch at index {0}")]
        DescriptorMismatch(u32),
        #[error(
            "manifest capabilities advertise FEATURE_ENTROPY_BUNDLED={found_bundled} but encoder requires {required_bundled}"
        )]
        CapabilityEntropyFlagMismatch {
            /// Whether the encoder expects bundled entropy.
            required_bundled: bool,
            /// Whether the manifest set the bundled capability bit.
            found_bundled: bool,
        },
        #[error(
            "manifest capabilities advertise acceleration mask {found_mask:#06x} but entropy mode {entropy_mode:?} requires mask {required_mask:#06x}"
        )]
        CapabilityAccelerationFlagMismatch {
            /// Manifest entropy mode under verification.
            entropy_mode: EntropyMode,
            /// Acceleration flag mask detected in the manifest.
            found_mask: u32,
            /// Acceleration mask required by the encoder configuration.
            required_mask: u32,
        },
    }

    /// Frame geometry used by the baseline codec (luma-only for now).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct FrameDimensions {
        pub width: u16,
        pub height: u16,
    }

    impl FrameDimensions {
        pub const fn new(width: u16, height: u16) -> Self {
            Self { width, height }
        }

        pub const fn pixel_count(self) -> usize {
            self.width as usize * self.height as usize
        }

        const fn align_component(value: u16) -> u16 {
            if value == 0 {
                0
            } else {
                let value_usize = value as usize;
                let aligned = value_usize.div_ceil(BLOCK_SIZE).saturating_mul(BLOCK_SIZE);
                if aligned > u16::MAX as usize {
                    u16::MAX
                } else {
                    aligned as u16
                }
            }
        }

        pub const fn align_to_block(self) -> Self {
            Self {
                width: Self::align_component(self.width),
                height: Self::align_component(self.height),
            }
        }

        pub const fn block_aligned(self) -> bool {
            (self.width as usize).is_multiple_of(BLOCK_SIZE)
                && (self.height as usize).is_multiple_of(BLOCK_SIZE)
        }

        pub const fn block_count(self) -> usize {
            (self.width as usize / BLOCK_SIZE) * (self.height as usize / BLOCK_SIZE)
        }
    }

    /// Raw luma frame used by the baseline encoder.
    #[derive(Clone, Debug, PartialEq, Eq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct RawFrame {
        pub luma: Vec<u8>,
    }

    impl RawFrame {
        pub fn new(dimensions: FrameDimensions, luma: Vec<u8>) -> Result<Self, CodecError> {
            let expected = dimensions.pixel_count();
            if luma.len() != expected {
                return Err(CodecError::InvalidFrameLength(FrameLengthMismatch {
                    expected: saturating_usize_to_u32(expected),
                    actual: saturating_usize_to_u32(luma.len()),
                }));
            }
            Ok(Self { luma })
        }
    }

    /// 4:2:0 chroma planes paired with a luma frame.
    #[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct Chroma420Frame {
        /// U plane in row-major order.
        pub u: Bytes,
        /// V plane in row-major order.
        pub v: Bytes,
    }

    pub(crate) fn ensure_chroma_even_dimensions(
        dimensions: FrameDimensions,
    ) -> Result<(), CodecError> {
        if !dimensions.width.is_multiple_of(2) || !dimensions.height.is_multiple_of(2) {
            return Err(CodecError::ChromaDimensionsNotEven(
                ChromaDimensionsNotEvenInfo {
                    width: dimensions.width,
                    height: dimensions.height,
                },
            ));
        }
        Ok(())
    }

    impl Chroma420Frame {
        /// Construct a chroma frame, validating that both planes match the expected dimensions.
        pub fn new(dimensions: FrameDimensions, u: Bytes, v: Bytes) -> Result<Self, CodecError> {
            ensure_chroma_even_dimensions(dimensions)?;
            let expected = usize::from(dimensions.width / 2) * usize::from(dimensions.height / 2);
            if u.len() != expected || v.len() != expected {
                return Err(CodecError::InvalidFrameLength(FrameLengthMismatch {
                    expected: saturating_usize_to_u32(expected),
                    actual: saturating_usize_to_u32(u.len().max(v.len())),
                }));
            }
            Ok(Self { u, v })
        }

        /// Build a neutral 4:2:0 chroma frame (U/V filled with 128).
        pub fn neutral(dimensions: FrameDimensions) -> Self {
            debug_assert!(
                dimensions.width.is_multiple_of(2) && dimensions.height.is_multiple_of(2),
                "4:2:0 chroma requires even dimensions"
            );
            let expected = usize::from(dimensions.width / 2) * usize::from(dimensions.height / 2);
            let plane = vec![128u8; expected];
            Self {
                u: plane.clone(),
                v: plane,
            }
        }
    }

    const AUDIO_SYNC_TOLERANCE_NS: u64 = 10_000_000;

    fn checked_frame_count(frame_count: usize) -> Result<u16, CodecError> {
        u16::try_from(frame_count).map_err(|_| {
            CodecError::FrameCountOverflow(FrameCountOverflowInfo {
                max: u32::from(u16::MAX),
                found: saturating_usize_to_u32(frame_count),
            })
        })
    }

    fn expected_audio_frame_samples(
        sample_rate: u32,
        frame_duration_ns: u32,
    ) -> Result<u16, CodecError> {
        const NS_PER_SEC: u128 = 1_000_000_000;
        let numerator = u128::from(sample_rate) * u128::from(frame_duration_ns);
        let expected = (numerator + NS_PER_SEC / 2) / NS_PER_SEC;
        if expected > u16::MAX as u128 {
            return Err(CodecError::AudioFrameCadenceOverflow(
                AudioFrameCadenceOverflowInfo {
                    expected: expected as u64,
                    sample_rate,
                    frame_duration_ns,
                },
            ));
        }
        Ok(expected as u16)
    }

    struct AudioEncoderState {
        encoder: iroha_audio::Encoder,
        sequence: u64,
    }

    pub struct BaselineEncoder {
        config: BaselineEncoderConfig,
        audio: Option<AudioEncoderState>,
        bundled_telemetry: Option<BundledTelemetry>,
        bundle_tables: Arc<BundleAnsTables>,
        rdo_lambda: f32,
        neural_predictor: Option<NeuralPredictor>,
    }

    pub(crate) fn pad_frame_luma(
        input: &[u8],
        dims: FrameDimensions,
        aligned: FrameDimensions,
    ) -> Vec<u8> {
        if dims == aligned {
            return input.to_vec();
        }
        let dst_w = aligned.width as usize;
        let dst_h = aligned.height as usize;
        let mut out = vec![0u8; dst_w * dst_h];
        let src_w = dims.width as usize;
        let src_h = dims.height as usize;
        if src_w == 0 || src_h == 0 {
            return out;
        }
        for y in 0..src_h {
            let src_row = &input[y * src_w..(y + 1) * src_w];
            let dst_row = &mut out[y * dst_w..(y + 1) * dst_w];
            dst_row[..src_w].copy_from_slice(src_row);
            let fill = src_row.last().copied().unwrap_or(0);
            for value in &mut dst_row[src_w..] {
                *value = fill;
            }
        }
        if src_h < dst_h {
            let last_row = out[(src_h - 1) * dst_w..src_h * dst_w].to_vec();
            for y in src_h..dst_h {
                let dst_row = &mut out[y * dst_w..(y + 1) * dst_w];
                dst_row.copy_from_slice(&last_row);
            }
        }
        out
    }

    pub(crate) fn crop_frame_luma(
        aligned_data: &[u8],
        dims: FrameDimensions,
        aligned: FrameDimensions,
    ) -> Vec<u8> {
        if dims == aligned {
            return aligned_data.to_vec();
        }
        let dst_w = dims.width as usize;
        let dst_h = dims.height as usize;
        if dst_w == 0 || dst_h == 0 {
            return Vec::new();
        }
        let src_w = aligned.width as usize;
        let mut out = Vec::with_capacity(dst_w * dst_h);
        for y in 0..dst_h {
            let start = y * src_w;
            out.extend_from_slice(&aligned_data[start..start + dst_w]);
        }
        out
    }

    impl BaselineEncoder {
        pub fn new(config: BaselineEncoderConfig) -> Self {
            assert!(
                config.entropy_mode.is_bundled(),
                "rANS entropy mode without bundled tables is not supported in the streaming baseline encoder"
            );
            assert!(
                config.bundle_width >= 2,
                "bundled rANS requires bundle_width >= 2"
            );
            let bundle_tables = config.bundle_tables.clone();
            let neural_seed = config.rdo_neural_seed.unwrap_or(DEFAULT_NEURAL_SEED);
            let neural_predictor = (config.rdo_mode == RdoMode::Neural)
                .then(|| NeuralPredictor::from_seed(neural_seed));
            let rdo_lambda = rdo_lambda_for_mode(config.rdo_mode, config.quantizer);
            Self {
                bundle_tables,
                config,
                audio: None,
                bundled_telemetry: None,
                rdo_lambda,
                neural_predictor,
            }
        }

        /// Latest bundled entropy telemetry (stats + token stream) recorded by the encoder.
        #[must_use]
        pub fn bundled_telemetry(&self) -> Option<&BundledTelemetry> {
            self.bundled_telemetry.as_ref()
        }

        fn ensure_audio_state(
            &mut self,
            audio_cfg: &AudioEncoderConfig,
        ) -> Result<&mut AudioEncoderState, AudioCodecError> {
            if self.audio.is_none() {
                let encoder = iroha_audio::Encoder::new(iroha_audio::EncoderConfig {
                    sample_rate: audio_cfg.sample_rate,
                    frame_samples: audio_cfg.frame_samples,
                    layout: audio_cfg.layout.into(),
                    fec_level: audio_cfg.fec_level,
                    target_bitrate: audio_cfg.target_bitrate,
                    backend: audio_cfg.backend,
                })
                .map_err(AudioCodecError::from_backend)?;

                self.audio = Some(AudioEncoderState {
                    encoder,
                    sequence: 0,
                });
            }
            Ok(self.audio.as_mut().expect("audio state initialized"))
        }

        fn encode_audio_track(
            &mut self,
            audio_pcm: Option<&[i16]>,
            frame_count: usize,
            timeline_start_ns: u64,
            frame_step: u64,
        ) -> Result<Option<SegmentAudio>, CodecError> {
            let Some(audio_cfg) = self.config.audio else {
                if audio_pcm.is_some() {
                    return Err(CodecError::AudioTrackUnexpected);
                }
                return Ok(None);
            };
            if audio_cfg.frame_samples == 0 {
                return Err(CodecError::Audio(AudioCodecError::InvalidSampleCount(
                    AudioEncoderSampleCountMismatchInfo {
                        expected: 1,
                        found: 0,
                    },
                )));
            }
            let expected_frame_samples =
                expected_audio_frame_samples(audio_cfg.sample_rate, self.config.frame_duration_ns)?;
            if audio_cfg.frame_samples != expected_frame_samples {
                return Err(CodecError::AudioFrameCadenceMismatch(
                    AudioFrameCadenceMismatchInfo {
                        expected: expected_frame_samples,
                        found: audio_cfg.frame_samples,
                        sample_rate: audio_cfg.sample_rate,
                        frame_duration_ns: self.config.frame_duration_ns,
                    },
                ));
            }

            let samples = audio_pcm.ok_or(CodecError::AudioTrackMissing)?;
            let channels = audio_cfg.channel_count();
            let samples_per_frame = audio_cfg.frame_samples as usize * channels;
            let expected_samples = frame_count
                .checked_mul(samples_per_frame)
                .ok_or(CodecError::AudioSampleCountOverflow)?;
            if samples.len() != expected_samples {
                return Err(CodecError::AudioSampleCountMismatch(
                    AudioSampleCountMismatchInfo {
                        expected: saturating_usize_to_u64(expected_samples),
                        found: saturating_usize_to_u64(samples.len()),
                    },
                ));
            }

            let state = self
                .ensure_audio_state(&audio_cfg)
                .map_err(CodecError::from)?;

            let mut frames_out = Vec::with_capacity(frame_count);
            for (idx, window) in samples.chunks(samples_per_frame).enumerate() {
                let payload = state
                    .encoder
                    .encode(window)
                    .map_err(AudioCodecError::from_backend)
                    .map_err(CodecError::from)?;
                let offset = frame_step.checked_mul(idx as u64).ok_or(
                    CodecError::AudioTimestampOverflow(saturating_usize_to_u32(idx)),
                )?;
                let timestamp = timeline_start_ns.checked_add(offset).ok_or(
                    CodecError::AudioTimestampOverflow(saturating_usize_to_u32(idx)),
                )?;
                frames_out.push(AudioFrame {
                    sequence: state.sequence,
                    timestamp_ns: timestamp,
                    fec_level: audio_cfg.fec_level,
                    channel_layout: audio_cfg.layout,
                    payload,
                });
                state.sequence = state
                    .sequence
                    .checked_add(1)
                    .ok_or(CodecError::AudioSequenceOverflow)?;
            }

            let frames_per_segment =
                u16::try_from(frame_count).map_err(|_| CodecError::AudioFrameCountOverflow)?;

            let summary = AudioTrackSummary {
                sample_rate: audio_cfg.sample_rate,
                frame_samples: audio_cfg.frame_samples,
                frame_duration_ns: self.config.frame_duration_ns,
                frames_per_segment,
                layout: audio_cfg.layout,
                fec_level: audio_cfg.fec_level,
            };

            Ok(Some(SegmentAudio {
                summary,
                frames: frames_out,
            }))
        }

        pub fn encode_segment(
            &mut self,
            segment_number: u64,
            timeline_start_ns: u64,
            content_key_id: u64,
            frames: &[RawFrame],
            audio_pcm: Option<&[i16]>,
        ) -> Result<EncodedSegment, CodecError> {
            self.encode_segment_with_chroma(
                segment_number,
                timeline_start_ns,
                content_key_id,
                frames,
                None,
                audio_pcm,
            )
        }

        pub fn encode_segment_with_chroma(
            &mut self,
            segment_number: u64,
            timeline_start_ns: u64,
            content_key_id: u64,
            frames: &[RawFrame],
            chroma: Option<&[Chroma420Frame]>,
            audio_pcm: Option<&[i16]>,
        ) -> Result<EncodedSegment, CodecError> {
            if frames.is_empty() {
                return Err(CodecError::Segment(SegmentError::Chunk(
                    ChunkError::EmptyTree,
                )));
            }
            let frame_count = checked_frame_count(frames.len())?;
            if let Some(chroma_frames) = chroma
                && chroma_frames.len() != frames.len()
            {
                return Err(CodecError::InvalidFrameLength(FrameLengthMismatch {
                    expected: saturating_usize_to_u32(frames.len()),
                    actual: saturating_usize_to_u32(chroma_frames.len()),
                }));
            }
            if chroma.is_some() {
                ensure_chroma_even_dimensions(self.config.frame_dimensions)?;
            }

            let dims = self.config.frame_dimensions;
            let frame_len = dims.pixel_count();
            let aligned_dims = dims.align_to_block();
            let blocks_per_frame = aligned_dims.block_count();
            let chunk_count_u16: u16 = blocks_per_frame.try_into().map_err(|_| {
                CodecError::BlockCountMismatch(BlockCountMismatchInfo {
                    expected: saturating_usize_to_u32(blocks_per_frame),
                    found: u32::MAX,
                })
            })?;
            let frame_step = u64::from(self.config.frame_duration_ns);

            let chroma_frames = chroma.unwrap_or(&[]);
            let chroma_dims = FrameDimensions::new(dims.width / 2, dims.height / 2);
            let chroma_aligned = chroma_dims.align_to_block();

            let mut chunks = Vec::with_capacity(frames.len());
            let mut reconstructed_prev: Option<Vec<u8>> = None;
            let mut previous_chroma_u: Option<Vec<u8>> = None;
            let mut previous_chroma_v: Option<Vec<u8>> = None;
            let mut bundler = self.config.entropy_mode.is_bundled().then(|| {
                let width = self
                    .config
                    .bundle_width
                    .min(self.bundle_tables.max_width())
                    .max(1);
                BundleStreamRecorder::new(
                    width,
                    aligned_dims,
                    self.config.quantizer,
                    self.bundle_tables.clone(),
                    self.config.bundle_acceleration,
                    self.config.bundle_prefetch_distance,
                    self.config.bundle_context_remap.clone(),
                )
            });
            let mut rdo_builder =
                RdoTelemetryBuilder::new(self.config.rdo_mode, self.rdo_lambda, NEURAL_OUTPUT);
            let mut noop_hooks = NoopBundledHooks;
            for (idx, frame) in frames.iter().enumerate() {
                if frame.luma.len() != frame_len {
                    return Err(CodecError::InvalidFrameLength(FrameLengthMismatch {
                        expected: saturating_usize_to_u32(frame_len),
                        actual: saturating_usize_to_u32(frame.luma.len()),
                    }));
                }

                let padded_storage = if dims == aligned_dims {
                    None
                } else {
                    Some(pad_frame_luma(&frame.luma, dims, aligned_dims))
                };
                let frame_slice: &[u8] = if let Some(ref buf) = padded_storage {
                    buf.as_slice()
                } else {
                    frame.luma.as_slice()
                };

                let frame_type = if idx == 0 {
                    FrameType::Intra
                } else {
                    FrameType::Predicted
                };
                let idx_delta = frame_step
                    .checked_mul(idx as u64)
                    .ok_or(CodecError::FramePtsOverflow(saturating_usize_to_u32(idx)))?;
                let pts = timeline_start_ns
                    .checked_add(idx_delta)
                    .ok_or(CodecError::FramePtsOverflow(saturating_usize_to_u32(idx)))?;

                let prev_ref = reconstructed_prev.as_deref();
                let hooks: &mut dyn BundledHooks = bundler
                    .as_mut()
                    .map(|rec| rec as &mut dyn BundledHooks)
                    .unwrap_or(&mut noop_hooks);
                let (payload, recon) = Self::encode_frame_payload(
                    frame_slice,
                    frame_type,
                    prev_ref,
                    aligned_dims,
                    self.config.quantizer,
                    hooks,
                    rdo_builder.as_mut(),
                    self.neural_predictor.as_ref(),
                    self.config.bundle_width,
                    self.config.rdo_mode,
                )?;

                let chroma_planes = chroma_frames.get(idx);
                let mut chunk = Vec::with_capacity(
                    FRAME_HEADER_LEN
                        + payload.len()
                        + chroma_planes
                            .map(|planes| planes.u.len().saturating_add(planes.v.len()) + 8)
                            .unwrap_or(0),
                );
                chunk.extend_from_slice(&(idx as u32).to_le_bytes());
                chunk.extend_from_slice(&pts.to_le_bytes());
                chunk.push(frame_type.as_byte());
                chunk.push(self.config.quantizer);
                chunk.extend_from_slice(&chunk_count_u16.to_le_bytes());
                chunk.extend_from_slice(&payload);
                if let Some(planes) = chroma_planes {
                    let expected_chroma_len = chroma_dims.pixel_count();
                    if planes.u.len() != expected_chroma_len
                        || planes.v.len() != expected_chroma_len
                    {
                        return Err(CodecError::InvalidFrameLength(FrameLengthMismatch {
                            expected: saturating_usize_to_u32(expected_chroma_len),
                            actual: saturating_usize_to_u32(planes.u.len().max(planes.v.len())),
                        }));
                    }
                    let padded_u = if chroma_dims == chroma_aligned {
                        None
                    } else {
                        Some(pad_frame_luma(&planes.u, chroma_dims, chroma_aligned))
                    };
                    let padded_v = if chroma_dims == chroma_aligned {
                        None
                    } else {
                        Some(pad_frame_luma(&planes.v, chroma_dims, chroma_aligned))
                    };
                    let (u_payload, u_recon) = Self::encode_frame_payload(
                        padded_u
                            .as_ref()
                            .map_or_else(|| planes.u.as_slice(), Vec::as_slice),
                        frame_type,
                        previous_chroma_u.as_deref(),
                        chroma_aligned,
                        self.config.quantizer,
                        &mut noop_hooks,
                        None,
                        None,
                        1,
                        RdoMode::None,
                    )?;
                    let (v_payload, v_recon) = Self::encode_frame_payload(
                        padded_v
                            .as_ref()
                            .map_or_else(|| planes.v.as_slice(), Vec::as_slice),
                        frame_type,
                        previous_chroma_v.as_deref(),
                        chroma_aligned,
                        self.config.quantizer,
                        &mut noop_hooks,
                        None,
                        None,
                        1,
                        RdoMode::None,
                    )?;
                    previous_chroma_u = Some(u_recon);
                    previous_chroma_v = Some(v_recon);
                    chunk.extend_from_slice(&(u_payload.len() as u32).to_le_bytes());
                    chunk.extend_from_slice(&(v_payload.len() as u32).to_le_bytes());
                    chunk.extend_from_slice(&u_payload);
                    chunk.extend_from_slice(&v_payload);
                } else {
                    previous_chroma_u = None;
                    previous_chroma_v = None;
                }
                chunks.push(chunk);

                reconstructed_prev = Some(recon);
            }

            let rdo_report = rdo_builder.and_then(RdoTelemetryBuilder::finish);
            self.bundled_telemetry = bundler.map(|rec| {
                let mut telemetry = rec.finish();
                telemetry.rdo = rdo_report;
                telemetry
            });

            let offsets_and_lengths = compute_offsets(&chunks);
            let chunk_ids: Vec<u16> = (0..frame_count).collect();

            let nonce_salt = derive_nonce_salt(segment_number, frames.len(), &chunks);

            let payload_refs: Vec<(u16, &[u8])> = chunk_ids
                .iter()
                .zip(chunks.iter())
                .map(|(id, payload)| (*id, payload.as_slice()))
                .collect();
            let commitments = chunk_commitments(segment_number, &payload_refs);
            let root = merkle_root(&commitments)
                .map_err(SegmentError::from)
                .map_err(CodecError::from)?;

            let audio =
                self.encode_audio_track(audio_pcm, frames.len(), timeline_start_ns, frame_step)?;

            let descriptors: Vec<ChunkDescriptor> = offsets_and_lengths
                .into_iter()
                .zip(commitments.iter())
                .zip(chunk_ids.iter())
                .map(|((meta, commitment), chunk_id)| ChunkDescriptor {
                    chunk_id: *chunk_id,
                    offset: meta.offset,
                    length: meta.length,
                    commitment: *commitment,
                    parity: false,
                })
                .collect();

            let entropy_tables_checksum = self
                .config
                .entropy_mode
                .is_bundled()
                .then(|| self.bundle_tables.checksum());

            let header = SegmentHeader {
                segment_number,
                profile: self.config.profile,
                entropy_mode: self.config.entropy_mode,
                entropy_tables_checksum,
                encryption_suite: self.config.encryption_suite,
                layer_bitmap: self.config.layer_bitmap,
                chunk_merkle_root: root,
                chunk_count: frame_count,
                timeline_start_ns,
                duration_ns: if self.config.duration_ns == 0 {
                    self.config
                        .frame_duration_ns
                        .saturating_mul(frames.len() as u32)
                        .max(1)
                } else {
                    self.config.duration_ns
                },
                feedback_hint: self.config.feedback_hint.clone(),
                content_key_id,
                nonce_salt,
                storage_class: self.config.storage_class,
                audio_summary: audio.as_ref().map(|track| track.summary),
                bundle_acceleration: self.config.bundle_acceleration,
            };

            Ok(EncodedSegment {
                header,
                descriptors,
                chunks,
                audio,
            })
        }

        #[allow(clippy::too_many_arguments)]
        fn encode_frame_payload(
            frame: &[u8],
            frame_type: FrameType,
            prev_frame: Option<&[u8]>,
            dims: FrameDimensions,
            quantizer: u8,
            hooks: &mut dyn BundledHooks,
            mut rdo: Option<&mut RdoTelemetryBuilder>,
            neural_predictor: Option<&NeuralPredictor>,
            bundle_width: u8,
            rdo_mode: RdoMode,
        ) -> Result<(Vec<u8>, Vec<u8>), CodecError> {
            debug_assert_eq!(frame.len(), dims.pixel_count());
            let mut payload = Vec::with_capacity(frame.len() / 2);
            let mut reconstructed = vec![0u8; dims.pixel_count()];
            let mut prev_dc = 0i16;
            if frame_type.reset_dc() {
                prev_dc = 0;
            }

            for block_index in 0..dims.block_count() {
                let (residual, predictor) =
                    build_residual_block(frame, prev_frame, dims, block_index, frame_type);
                let coeffs = forward_dct(&residual);
                let mut quantized = quantize_coeffs(&coeffs, quantizer);
                if let Some(builder) = rdo.as_deref_mut() {
                    let report = optimize_block_dp(
                        &mut quantized,
                        rdo_mode,
                        builder.lambda_bits,
                        neural_predictor,
                        bundle_width,
                        quantizer,
                        frame_type,
                        block_index,
                    );
                    builder.record(&report);
                }
                encode_block_rle(
                    &quantized,
                    &mut prev_dc,
                    &mut payload,
                    hooks,
                    block_index,
                    frame_type,
                );

                let dequantized = dequantize_coeffs(&quantized, quantizer);
                let spatial = inverse_dct(&dequantized);
                write_reconstructed_block(
                    &mut reconstructed,
                    &spatial,
                    &predictor,
                    dims,
                    block_index,
                );
            }

            Ok((payload, reconstructed))
        }
    }

    pub(crate) fn block_origin(dims: FrameDimensions, block_index: usize) -> (usize, usize) {
        let blocks_per_row = dims.width as usize / BLOCK_SIZE;
        let row = block_index / blocks_per_row;
        let col = block_index % blocks_per_row;
        (col * BLOCK_SIZE, row * BLOCK_SIZE)
    }

    pub(crate) fn build_residual_block(
        frame: &[u8],
        prev_frame: Option<&[u8]>,
        dims: FrameDimensions,
        block_index: usize,
        frame_type: FrameType,
    ) -> ([i16; BLOCK_PIXELS], [i16; BLOCK_PIXELS]) {
        let mut residual = [0i16; BLOCK_PIXELS];
        let mut predictor = [0i16; BLOCK_PIXELS];
        let (origin_x, origin_y) = block_origin(dims, block_index);
        let stride = dims.width as usize;
        for y in 0..BLOCK_SIZE {
            for x in 0..BLOCK_SIZE {
                let idx = (origin_y + y) * stride + (origin_x + x);
                let pixel = frame[idx] as i16;
                let pred = if let (FrameType::Predicted, Some(prev)) = (frame_type, prev_frame) {
                    prev[idx] as i16
                } else {
                    0
                };
                predictor[y * BLOCK_SIZE + x] = pred;
                residual[y * BLOCK_SIZE + x] = pixel - pred;
            }
        }
        (residual, predictor)
    }

    pub(crate) fn predictor_block(
        prev_frame: Option<&[u8]>,
        dims: FrameDimensions,
        block_index: usize,
        frame_type: FrameType,
    ) -> [i16; BLOCK_PIXELS] {
        if let (FrameType::Predicted, Some(prev)) = (frame_type, prev_frame) {
            let mut predictor = [0i16; BLOCK_PIXELS];
            let (origin_x, origin_y) = block_origin(dims, block_index);
            let stride = dims.width as usize;
            for y in 0..BLOCK_SIZE {
                for x in 0..BLOCK_SIZE {
                    let idx = (origin_y + y) * stride + (origin_x + x);
                    predictor[y * BLOCK_SIZE + x] = prev[idx] as i16;
                }
            }
            predictor
        } else {
            [0i16; BLOCK_PIXELS]
        }
    }

    pub(crate) fn write_reconstructed_block(
        reconstructed: &mut [u8],
        spatial: &[i32; BLOCK_PIXELS],
        predictor: &[i16; BLOCK_PIXELS],
        dims: FrameDimensions,
        block_index: usize,
    ) {
        let (origin_x, origin_y) = block_origin(dims, block_index);
        let stride = dims.width as usize;
        for y in 0..BLOCK_SIZE {
            for x in 0..BLOCK_SIZE {
                let idx = (origin_y + y) * stride + (origin_x + x);
                let sample_idx = y * BLOCK_SIZE + x;
                let predicted = predictor[sample_idx] as i32;
                reconstructed[idx] = clamp_pixel(spatial[sample_idx] + predicted);
            }
        }
    }

    #[cfg(test)]
    mod block_tests {
        use super::*;

        fn linear_frame(dims: FrameDimensions, offset: u8) -> Vec<u8> {
            (0..dims.pixel_count())
                .map(|idx| offset.wrapping_add(idx as u8))
                .collect()
        }

        fn block_pixels(frame: &[u8], dims: FrameDimensions, block_index: usize) -> Vec<u8> {
            let (origin_x, origin_y) = block_origin(dims, block_index);
            let stride = dims.width as usize;
            let mut out = Vec::with_capacity(BLOCK_PIXELS);
            for y in 0..BLOCK_SIZE {
                for x in 0..BLOCK_SIZE {
                    let idx = (origin_y + y) * stride + (origin_x + x);
                    out.push(frame[idx]);
                }
            }
            out
        }

        #[test]
        fn block_origin_maps_index_to_coordinates() {
            let dims = FrameDimensions::new(16, 16);
            assert_eq!(block_origin(dims, 0), (0, 0));
            assert_eq!(block_origin(dims, 1), (BLOCK_SIZE, 0));
            assert_eq!(block_origin(dims, 2), (0, BLOCK_SIZE));
            assert_eq!(block_origin(dims, 3), (BLOCK_SIZE, BLOCK_SIZE));
        }

        #[test]
        fn build_residual_block_tracks_previous_frame_for_predicted() {
            let dims = FrameDimensions::new(16, 8);
            let prev = linear_frame(dims, 5);
            let current: Vec<u8> = prev.iter().map(|value| value.saturating_add(2)).collect();
            let (residual, predictor) =
                build_residual_block(&current, Some(&prev), dims, 1, FrameType::Predicted);

            assert!(residual.iter().all(|&value| value == 2));
            let expected_block = block_pixels(&prev, dims, 1);
            for (actual, expected) in predictor.iter().zip(expected_block) {
                assert_eq!(*actual, i16::from(expected));
            }
        }

        #[test]
        fn build_residual_block_resets_predictor_for_intra_frames() {
            let dims = FrameDimensions::new(8, 8);
            let current = linear_frame(dims, 10);
            let prev = linear_frame(dims, 3);
            let (residual, predictor) =
                build_residual_block(&current, Some(&prev), dims, 0, FrameType::Intra);

            assert!(predictor.iter().all(|&value| value == 0));
            for (idx, &value) in residual.iter().enumerate() {
                assert_eq!(value, i16::from(current[idx]));
            }
        }

        #[test]
        fn predictor_block_only_uses_previous_frame_for_predicted() {
            let dims = FrameDimensions::new(16, 8);
            let prev = linear_frame(dims, 7);

            let predicted = predictor_block(Some(&prev), dims, 1, FrameType::Predicted);
            let expected_block = block_pixels(&prev, dims, 1);
            for (actual, expected) in predicted.iter().zip(expected_block) {
                assert_eq!(*actual, i16::from(expected));
            }

            let intra = predictor_block(Some(&prev), dims, 1, FrameType::Intra);
            assert!(intra.iter().all(|&value| value == 0));
        }

        #[test]
        fn write_reconstructed_block_clamps_and_combines_samples() {
            let dims = FrameDimensions::new(8, 8);
            let mut reconstructed = vec![0u8; dims.pixel_count()];
            let mut spatial = [20i32; BLOCK_PIXELS];
            spatial[0] = 400;
            spatial[1] = -400;
            let mut predictor = [10i16; BLOCK_PIXELS];
            predictor[2] = 5;
            write_reconstructed_block(&mut reconstructed, &spatial, &predictor, dims, 0);
            assert_eq!(reconstructed[0], 255, "values above 255 must clamp");
            assert_eq!(reconstructed[1], 0, "values below 0 must clamp");
            assert_eq!(reconstructed[2], 25);
            assert_eq!(reconstructed[3], 30);
        }

        #[cfg(feature = "streaming-neural-filter")]
        #[test]
        fn neural_filter_is_deterministic_and_effectful() {
            let dims = FrameDimensions::new(4, 4);
            let mut frame: Vec<u8> = (0..dims.pixel_count())
                .map(|idx| (idx as u8).saturating_mul(3))
                .collect();
            let mut second = frame.clone();
            apply_neural_filter(&mut frame, dims);
            apply_neural_filter(&mut second, dims);
            assert_eq!(frame, second, "neural filter must be deterministic");
            let untouched: Vec<u8> = (0..dims.pixel_count())
                .map(|idx| (idx as u8).saturating_mul(3))
                .collect();
            assert_ne!(frame, untouched, "neural filter should change the frame");
        }
    }

    /// TODO: consider SIMD/GPU acceleration for the fixed-point DCT path.
    #[cfg(feature = "streaming-fixed-point-dct")]
    pub(crate) fn forward_dct(block: &[i16; BLOCK_PIXELS]) -> [i32; BLOCK_PIXELS] {
        let mut out = [0i32; BLOCK_PIXELS];
        for u in 0..8 {
            for v in 0..8 {
                let mut sum: i64 = 0;
                for x in 0..8 {
                    for y in 0..8 {
                        let sample = i64::from(block[x * 8 + y]);
                        let factor_u = i64::from(DCT_FACTORS_Q15[u][x]);
                        let factor_v = i64::from(DCT_FACTORS_Q15[v][y]);
                        sum = sum.saturating_add(
                            sample.saturating_mul(factor_u).saturating_mul(factor_v),
                        );
                    }
                }
                let rounded = if sum >= 0 {
                    (sum + DCT_ROUND) >> DCT_SHIFT
                } else {
                    (sum - DCT_ROUND) >> DCT_SHIFT
                };
                out[u * 8 + v] = rounded as i32;
            }
        }
        out
    }

    #[cfg(not(feature = "streaming-fixed-point-dct"))]
    pub(crate) fn forward_dct(block: &[i16; BLOCK_PIXELS]) -> [i32; BLOCK_PIXELS] {
        let mut out = [0i32; BLOCK_PIXELS];
        for u in 0..8 {
            for v in 0..8 {
                let mut sum = 0.0;
                for x in 0..8 {
                    for y in 0..8 {
                        let sample = block[x * 8 + y] as f64;
                        sum += sample * DCT_FACTORS[u][x] * DCT_FACTORS[v][y];
                    }
                }
                out[u * 8 + v] = sum.round() as i32;
            }
        }
        out
    }

    #[cfg(feature = "streaming-fixed-point-dct")]
    pub(crate) fn inverse_dct(coeffs: &[i32; BLOCK_PIXELS]) -> [i32; BLOCK_PIXELS] {
        let mut out = [0i32; BLOCK_PIXELS];
        for x in 0..8 {
            for y in 0..8 {
                let mut sum: i64 = 0;
                for u in 0..8 {
                    for v in 0..8 {
                        let coeff = i64::from(coeffs[u * 8 + v]);
                        let factor_u = i64::from(DCT_FACTORS_Q15[u][x]);
                        let factor_v = i64::from(DCT_FACTORS_Q15[v][y]);
                        sum = sum.saturating_add(
                            coeff.saturating_mul(factor_u).saturating_mul(factor_v),
                        );
                    }
                }
                let rounded = if sum >= 0 {
                    (sum + DCT_ROUND) >> DCT_SHIFT
                } else {
                    (sum - DCT_ROUND) >> DCT_SHIFT
                };
                out[x * 8 + y] = rounded.clamp(-32768, 32767) as i32;
            }
        }
        out
    }

    #[cfg(not(feature = "streaming-fixed-point-dct"))]
    pub(crate) fn inverse_dct(coeffs: &[i32; BLOCK_PIXELS]) -> [i32; BLOCK_PIXELS] {
        let mut out = [0i32; BLOCK_PIXELS];
        for x in 0..8 {
            for y in 0..8 {
                let mut sum = 0.0;
                for u in 0..8 {
                    for v in 0..8 {
                        let coeff = coeffs[u * 8 + v] as f64;
                        sum += coeff * DCT_FACTORS[u][x] * DCT_FACTORS[v][y];
                    }
                }
                out[x * 8 + y] = sum.round().clamp(-32768.0, 32767.0) as i32;
            }
        }
        out
    }

    fn qp_scale(quantizer: u8) -> i32 {
        (quantizer as i32).max(1)
    }

    pub(crate) fn quantize_coeffs(
        coeffs: &[i32; BLOCK_PIXELS],
        quantizer: u8,
    ) -> [i16; BLOCK_PIXELS] {
        let mut out = [0i16; BLOCK_PIXELS];
        if quantizer == 0 {
            for (idx, coeff) in coeffs.iter().enumerate() {
                out[idx] = (*coeff).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
            }
            return out;
        }

        let scale = qp_scale(quantizer);
        for (idx, coeff) in coeffs.iter().enumerate() {
            let step = (i32::from(BASELINE_QUANT_MATRIX[idx]).max(1)) * scale;
            let adjusted = if *coeff >= 0 {
                coeff.saturating_add(step / 2)
            } else {
                coeff.saturating_sub(step / 2)
            };
            let quantized = adjusted / step;
            out[idx] = quantized.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        }
        out
    }

    pub(crate) fn dequantize_coeffs(
        coeffs: &[i16; BLOCK_PIXELS],
        quantizer: u8,
    ) -> [i32; BLOCK_PIXELS] {
        let mut out = [0i32; BLOCK_PIXELS];
        if quantizer == 0 {
            for (idx, coeff) in coeffs.iter().enumerate() {
                out[idx] = i32::from(*coeff);
            }
            return out;
        }

        let scale = qp_scale(quantizer);
        for (idx, coeff) in coeffs.iter().enumerate() {
            let step = (i32::from(BASELINE_QUANT_MATRIX[idx]).max(1)) * scale;
            out[idx] = i32::from(*coeff) * step;
        }
        out
    }

    pub(crate) trait BundledHooks {
        fn record_dc(&mut self, _diff: i16) {}
        fn record_ac(&mut self, _zeros: u8, _value: i16) {}
        fn record_eob(&mut self) {}
        fn record_block_coeffs(
            &mut self,
            _coeffs: &[i16; BLOCK_PIXELS],
            _block_index: usize,
            _frame_type: FrameType,
        ) {
        }
    }

    #[derive(Default)]
    struct NoopBundledHooks;

    impl BundledHooks for NoopBundledHooks {}

    #[derive(Clone, Copy, Debug, Default, NoritoSerialize, NoritoDeserialize)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct BundledStats {
        pub bundle_width: u8,
        pub bundles_total: u64,
        pub blocks_encoded: u64,
        pub significance_zero: u64,
        pub significance_one: u64,
        pub sign_negative: u64,
        pub sign_positive: u64,
        pub parity_one: u64,
        pub geq2_one: u64,
        pub dc_events: u64,
        pub ac_events: u64,
        pub nonzero_levels: u64,
        pub zero_run_total: u64,
        pub max_zero_run: u8,
        #[norito(default)]
        pub significance_rle: u64,
        pub flush_type_complete: u64,
        pub flush_context_switch: u64,
        pub flush_end_of_block: u64,
    }

    impl BundledStats {
        fn record_flush(&mut self, reason: BundleFlushReason) {
            match reason {
                BundleFlushReason::TypeComplete => {
                    self.flush_type_complete = self.flush_type_complete.saturating_add(1);
                }
                BundleFlushReason::ContextSwitch => {
                    self.flush_context_switch = self.flush_context_switch.saturating_add(1);
                }
                BundleFlushReason::EndOfBlock => {
                    self.flush_end_of_block = self.flush_end_of_block.saturating_add(1);
                }
            }
        }
    }

    pub(crate) const RDO_BUCKET_COUNT: usize = 5;
    pub(crate) const RDO_MAX_SYMBOLS: usize = 1 << MAX_BUNDLE_WIDTH;

    #[derive(Clone, Debug, Default, NoritoSerialize, NoritoDeserialize)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct RdoTelemetry {
        pub mode: RdoMode,
        pub lambda_bits: f32,
        pub blocks_optimized: u64,
        pub energy_histogram: [u64; RDO_BUCKET_COUNT],
        pub before_rate_bits: u64,
        pub after_rate_bits: u64,
        pub distortion_penalty: u64,
        pub neural_class_histogram: Vec<u64>,
    }

    #[derive(Clone, Copy, Debug, Default, NoritoSerialize, NoritoDeserialize)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct ContextFrequency {
        pub context: BundleContextId,
        pub bundles: u64,
        pub total_bits: u64,
        pub symbol_counts: [u64; RDO_MAX_SYMBOLS],
    }

    /// Summary describing the context remap applied during bundle recording.
    #[derive(Clone, Copy, Debug, Default, NoritoSerialize, NoritoDeserialize)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct ContextRemapSummary {
        pub escape_context: BundleContextId,
        pub remapped: u32,
        pub dropped: u32,
    }

    #[derive(Clone, Debug, NoritoSerialize, NoritoDeserialize)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct BundledTelemetry {
        pub stats: BundledStats,
        pub tokens: Vec<BundledToken>,
        pub bundles: Vec<BundleRecord>,
        pub context_stats: Vec<BundleContextStats>,
        pub context_frequencies: Vec<ContextFrequency>,
        pub ans_stream: Vec<u8>,
        pub ans_precision_bits: u8,
        pub tables_checksum: Hash,
        #[norito(default)]
        pub acceleration: BundleAcceleration,
        #[norito(default)]
        pub prefetch_distance: u16,
        #[norito(skip_serializing_if = "Option::is_none")]
        #[norito(default)]
        pub context_remap: Option<ContextRemapSummary>,
        #[norito(skip_serializing_if = "Option::is_none")]
        #[norito(default)]
        pub rdo: Option<RdoTelemetry>,
    }

    impl BundledTelemetry {
        /// Decode the ANS stream into bundle symbols using the supplied tables.
        pub fn decode_symbols(
            &self,
            tables: &BundleAnsTables,
        ) -> Result<Vec<u8>, BundleDecodeError> {
            if self.tables_checksum != tables.checksum() {
                return Err(BundleDecodeError::ChecksumMismatch {
                    expected: self.tables_checksum,
                    found: tables.checksum(),
                });
            }
            decode_bundle_stream(&self.ans_stream, &self.bundles, tables)
        }

        /// Verify that the ANS stream reproduces the recorded bundle bits.
        pub fn verify_stream(&self, tables: &BundleAnsTables) -> Result<(), BundleDecodeError> {
            let decoded = self.decode_symbols(tables)?;
            for (idx, (record, decoded_bits)) in self.bundles.iter().zip(decoded.iter()).enumerate()
            {
                let mask = (1u8 << record.bit_len) - 1;
                let expected = record.bits & mask;
                let found = decoded_bits & mask;
                if expected != found {
                    return Err(BundleDecodeError::SymbolMismatch {
                        index: saturating_usize_to_u32(idx),
                        expected,
                        found,
                    });
                }
            }
            Ok(())
        }
    }

    #[derive(Clone, Copy, Debug, NoritoSerialize, NoritoDeserialize)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub enum BundledToken {
        DcDiff(i16),
        Ac { run: u8, value: i16 },
        EndOfBlock,
    }

    #[derive(Clone, Copy, Debug, NoritoSerialize, NoritoDeserialize)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub enum BundleType {
        SignificanceOnly,
        SignAndMagnitude,
        SignParity,
        SignParityLevel,
        SignificanceRle,
    }

    impl BundleType {
        const fn as_u8(self) -> u8 {
            match self {
                Self::SignificanceOnly => 0,
                Self::SignAndMagnitude => 1,
                Self::SignParity => 2,
                Self::SignParityLevel => 3,
                Self::SignificanceRle => 4,
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub enum BundleFlushReason {
        TypeComplete,
        ContextSwitch,
        EndOfBlock,
    }

    #[derive(
        Clone,
        Copy,
        Debug,
        Default,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        NoritoSerialize,
        NoritoDeserialize,
    )]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct BundleContextId(pub u16);

    impl BundleContextId {
        pub const fn new(raw: u16) -> Self {
            Self(raw)
        }
    }

    #[derive(Clone, Copy, Debug, NoritoSerialize, NoritoDeserialize)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct BundleRecord {
        pub bundle_type: BundleType,
        pub context: BundleContextId,
        pub bits: u8,
        pub bit_len: u8,
        pub flush: BundleFlushReason,
    }

    #[derive(Clone, Copy, Debug, NoritoSerialize, NoritoDeserialize)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct BundleContextStats {
        pub context: BundleContextId,
        pub bundles_total: u64,
        pub type_significance_only: u64,
        pub type_sign_and_magnitude: u64,
        pub type_sign_parity: u64,
        pub type_sign_parity_level: u64,
        #[norito(default)]
        pub type_significance_rle: u64,
        pub flush_type_complete: u64,
        pub flush_context_switch: u64,
        pub flush_end_of_block: u64,
    }

    impl BundleContextStats {
        const fn new(context: BundleContextId) -> Self {
            Self {
                context,
                bundles_total: 0,
                type_significance_only: 0,
                type_sign_and_magnitude: 0,
                type_sign_parity: 0,
                type_sign_parity_level: 0,
                type_significance_rle: 0,
                flush_type_complete: 0,
                flush_context_switch: 0,
                flush_end_of_block: 0,
            }
        }

        fn record_bundle(&mut self, bundle_type: BundleType, flush: BundleFlushReason) {
            self.bundles_total = self.bundles_total.saturating_add(1);
            match bundle_type {
                BundleType::SignificanceOnly => {
                    self.type_significance_only = self.type_significance_only.saturating_add(1);
                }
                BundleType::SignAndMagnitude => {
                    self.type_sign_and_magnitude = self.type_sign_and_magnitude.saturating_add(1);
                }
                BundleType::SignParity => {
                    self.type_sign_parity = self.type_sign_parity.saturating_add(1);
                }
                BundleType::SignParityLevel => {
                    self.type_sign_parity_level = self.type_sign_parity_level.saturating_add(1);
                }
                BundleType::SignificanceRle => {
                    self.type_significance_rle = self.type_significance_rle.saturating_add(1);
                }
            }
            self.record_flush(flush);
        }

        fn record_flush(&mut self, flush: BundleFlushReason) {
            match flush {
                BundleFlushReason::TypeComplete => {
                    self.flush_type_complete = self.flush_type_complete.saturating_add(1);
                }
                BundleFlushReason::ContextSwitch => {
                    self.flush_context_switch = self.flush_context_switch.saturating_add(1);
                }
                BundleFlushReason::EndOfBlock => {
                    self.flush_end_of_block = self.flush_end_of_block.saturating_add(1);
                }
            }
        }
    }

    #[derive(Clone, Copy)]
    struct DpReport {
        energy: u32,
        before_bits: u64,
        after_bits: u64,
        distortion_penalty: u64,
        neural_class: Option<usize>,
    }

    struct RdoTelemetryBuilder {
        mode: RdoMode,
        lambda_bits: f32,
        energy_histogram: [u64; RDO_BUCKET_COUNT],
        before_bits: u64,
        after_bits: u64,
        distortion_penalty: u64,
        neural_histogram: Vec<u64>,
        blocks: u64,
    }

    impl RdoTelemetryBuilder {
        fn new(mode: RdoMode, lambda_bits: f32, neural_classes: usize) -> Option<Self> {
            if !mode.is_enabled() {
                return None;
            }
            let mut neural_histogram = Vec::new();
            if matches!(mode, RdoMode::Neural) && neural_classes > 0 {
                neural_histogram.resize(neural_classes, 0);
            }
            Some(Self {
                mode,
                lambda_bits,
                energy_histogram: [0; RDO_BUCKET_COUNT],
                before_bits: 0,
                after_bits: 0,
                distortion_penalty: 0,
                neural_histogram,
                blocks: 0,
            })
        }

        fn record(&mut self, report: &DpReport) {
            let bucket = energy_bucket(report.energy);
            if let Some(slot) = self.energy_histogram.get_mut(bucket) {
                *slot = slot.saturating_add(1);
            }
            self.before_bits = self.before_bits.saturating_add(report.before_bits);
            self.after_bits = self.after_bits.saturating_add(report.after_bits);
            self.distortion_penalty = self
                .distortion_penalty
                .saturating_add(report.distortion_penalty);
            if let Some(class) = report.neural_class
                && let Some(slot) = self.neural_histogram.get_mut(class)
            {
                *slot = slot.saturating_add(1);
            }
            self.blocks = self.blocks.saturating_add(1);
        }

        fn finish(self) -> Option<RdoTelemetry> {
            Some(RdoTelemetry {
                mode: self.mode,
                lambda_bits: self.lambda_bits,
                blocks_optimized: self.blocks,
                energy_histogram: self.energy_histogram,
                before_rate_bits: self.before_bits,
                after_rate_bits: self.after_bits,
                distortion_penalty: self.distortion_penalty,
                neural_class_histogram: self.neural_histogram,
            })
        }
    }

    const fn energy_bucket(energy: u32) -> usize {
        let mut idx = 0usize;
        while idx < RDO_ENERGY_BUCKETS.len() {
            if energy <= RDO_ENERGY_BUCKETS[idx] {
                return idx;
            }
            idx += 1;
        }
        RDO_BUCKET_COUNT - 1
    }

    fn rdo_lambda_for_mode(mode: RdoMode, quantizer: u8) -> f32 {
        match mode {
            RdoMode::Perceptual => rdo_lambda_perceptual(quantizer),
            _ => rdo_lambda_for_quantizer(quantizer),
        }
    }

    fn rdo_lambda_for_quantizer(quantizer: u8) -> f32 {
        match quantizer_to_bucket(quantizer) {
            0 => 0.45,
            1 => 0.75,
            2 => 1.1,
            _ => 1.35,
        }
    }

    fn rdo_lambda_perceptual(quantizer: u8) -> f32 {
        // Slightly soften lambda at mid/high Q to favour structure (SSIM proxy) while
        // keeping low-Q output close to the rate-focused schedule.
        match quantizer_to_bucket(quantizer) {
            0 => 0.42,
            1 => 0.65,
            2 => 0.95,
            _ => 1.15,
        }
    }

    fn block_energy(coeffs: &[i16; BLOCK_PIXELS]) -> u32 {
        coeffs
            .iter()
            .enumerate()
            .skip(1)
            .map(|(_, &coeff)| coeff.unsigned_abs())
            .map(u32::from)
            .sum()
    }

    fn max_zero_run_in_block(coeffs: &[i16; BLOCK_PIXELS]) -> u8 {
        let mut max_run = 0u8;
        let mut run = 0u8;
        for &slot in ZIG_ZAG.iter().skip(1) {
            if coeffs[slot] == 0 {
                run = run.saturating_add(1);
                max_run = max_run.max(run);
            } else {
                run = 0;
            }
        }
        max_run
    }

    fn estimate_rle_bits(coeffs: &[i16; BLOCK_PIXELS]) -> u64 {
        let mut pos = 1usize;
        let mut bits = 0u64;
        while pos < BLOCK_PIXELS {
            let mut zero_run = 0usize;
            while pos < BLOCK_PIXELS && coeffs[ZIG_ZAG[pos]] == 0 {
                zero_run = zero_run.saturating_add(1);
                pos = pos.saturating_add(1);
            }
            if pos == BLOCK_PIXELS {
                bits = bits.saturating_add(RLE_TOKEN_BITS);
                break;
            }
            while zero_run > MAX_ZERO_RUN {
                bits = bits.saturating_add(RLE_TOKEN_BITS);
                zero_run = zero_run.saturating_sub(MAX_ZERO_RUN);
            }
            bits = bits.saturating_add(RLE_TOKEN_BITS);
            pos = pos.saturating_add(1);
        }
        bits
    }

    fn token_rate_bits(zero_run: usize) -> f32 {
        let mut tokens = 1u32;
        let mut run = zero_run;
        while run > MAX_ZERO_RUN {
            tokens = tokens.saturating_add(1);
            run = run.saturating_sub(MAX_ZERO_RUN);
        }
        tokens as f32 * RLE_TOKEN_BITS_F
    }

    #[allow(clippy::too_many_arguments)]
    fn optimize_block_dp(
        coeffs: &mut [i16; BLOCK_PIXELS],
        mode: RdoMode,
        lambda_bits: f32,
        predictor: Option<&NeuralPredictor>,
        bundle_width: u8,
        quantizer: u8,
        frame_type: FrameType,
        block_index: usize,
    ) -> DpReport {
        let original = *coeffs;
        let energy = block_energy(&original);
        if !mode.is_enabled() {
            return DpReport {
                energy,
                before_bits: estimate_rle_bits(&original),
                after_bits: estimate_rle_bits(coeffs),
                distortion_penalty: 0,
                neural_class: None,
            };
        }

        let mut lambda = lambda_bits;
        let mut neural_class = None;
        if matches!(mode, RdoMode::Neural)
            && let Some(net) = predictor
        {
            let features = build_neural_features(
                &original,
                energy,
                bundle_width,
                quantizer,
                frame_type,
                block_index,
            );
            let class_idx = net.predict(&features);
            neural_class = Some(class_idx);
            lambda *= match class_idx {
                0 => 0.85,
                1 => 1.0,
                2 => 1.12,
                _ => 1.28,
            };
        }

        let ac_len = BLOCK_PIXELS - 1;
        let mut best = vec![vec![f32::INFINITY; ac_len + 1]; ac_len + 1];
        let mut keep = vec![vec![false; ac_len + 1]; ac_len];

        for slot in best[ac_len].iter_mut().take(ac_len + 1) {
            *slot = RLE_EOB_BITS_F;
        }

        for idx in (0..ac_len).rev() {
            let coeff = original[ZIG_ZAG[idx + 1]];
            let distortion_zero = f32::from(coeff) * f32::from(coeff);
            for run in 0..=ac_len {
                let next_run = (run + 1).min(ac_len);
                let zero_cost = distortion_zero + best[idx + 1][next_run];
                let keep_cost = if coeff == 0 {
                    f32::INFINITY
                } else {
                    lambda * token_rate_bits(run) + best[idx + 1][0]
                };
                if keep_cost < zero_cost {
                    keep[idx][run] = true;
                    best[idx][run] = keep_cost;
                } else {
                    best[idx][run] = zero_cost;
                }
            }
        }

        let mut run = 0usize;
        for idx in 0..ac_len {
            let slot = ZIG_ZAG[idx + 1];
            if keep[idx][run] {
                run = 0;
            } else {
                coeffs[slot] = 0;
                run = (run + 1).min(ac_len);
            }
        }
        coeffs[0] = original[0];

        let before_bits = estimate_rle_bits(&original);
        let after_bits = estimate_rle_bits(coeffs);
        let distortion_penalty: u64 = original
            .iter()
            .zip(coeffs.iter())
            .skip(1)
            .map(|(&before, &after)| {
                let diff = i64::from(before) - i64::from(after);
                diff.saturating_mul(diff) as u64
            })
            .sum();

        DpReport {
            energy,
            before_bits,
            after_bits,
            distortion_penalty,
            neural_class,
        }
    }

    struct TinyPrng(u64);

    impl TinyPrng {
        fn new(seed: u64) -> Self {
            Self(seed | 1)
        }

        fn next(&mut self) -> u32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 32) as u32
        }

        fn next_i8(&mut self) -> i8 {
            let raw = self.next();
            let span = 11i16;
            let value = (raw % (2 * span as u32 + 1)) as i16 - span;
            value as i8
        }

        fn next_i16(&mut self) -> i16 {
            let raw = self.next();
            let span = 127i32;
            let value = (raw % (2 * span as u32 + 1)) as i32 - span;
            value as i16
        }
    }

    #[derive(Clone)]
    struct NeuralPredictor {
        weights1: [[i8; NEURAL_FEATURES]; NEURAL_HIDDEN],
        weights2: [[i8; NEURAL_HIDDEN]; NEURAL_OUTPUT],
        bias1: [i16; NEURAL_HIDDEN],
        bias2: [i16; NEURAL_OUTPUT],
        activation_scale: i16,
    }

    impl NeuralPredictor {
        fn from_seed(seed: [u8; 32]) -> Self {
            let mut seed_bytes = [0u8; 8];
            seed_bytes.copy_from_slice(&seed[..8]);
            let mut prng = TinyPrng::new(u64::from_le_bytes(seed_bytes));
            let mut weights1 = [[0i8; NEURAL_FEATURES]; NEURAL_HIDDEN];
            let mut weights2 = [[0i8; NEURAL_HIDDEN]; NEURAL_OUTPUT];
            let mut bias1 = [0i16; NEURAL_HIDDEN];
            let mut bias2 = [0i16; NEURAL_OUTPUT];
            for row in &mut weights1 {
                for weight in row {
                    *weight = prng.next_i8();
                }
            }
            for row in &mut weights2 {
                for weight in row {
                    *weight = prng.next_i8();
                }
            }
            for bias in &mut bias1 {
                *bias = prng.next_i16();
            }
            for bias in &mut bias2 {
                *bias = prng.next_i16();
            }
            let activation_scale = (prng.next() % 7 + 9) as i16;
            Self {
                weights1,
                weights2,
                bias1,
                bias2,
                activation_scale,
            }
        }

        fn predict(&self, features: &[i8; NEURAL_FEATURES]) -> usize {
            let mut hidden = [0i16; NEURAL_HIDDEN];
            for (idx, (row, bias)) in self.weights1.iter().zip(self.bias1.iter()).enumerate() {
                let mut acc = i32::from(*bias);
                for (&w, &f) in row.iter().zip(features.iter()) {
                    acc = acc.saturating_add(i32::from(w) * i32::from(f));
                }
                let scaled = acc / i32::from(self.activation_scale.max(1));
                let relu = scaled.max(0).min(i32::from(i16::MAX));
                hidden[idx] = relu as i16;
            }

            let mut best = (0usize, i32::MIN);
            for (class, (row, bias)) in self.weights2.iter().zip(self.bias2.iter()).enumerate() {
                let mut acc = i32::from(*bias);
                for (&w, &h) in row.iter().zip(hidden.iter()) {
                    acc = acc.saturating_add(i32::from(w) * i32::from(h));
                }
                if acc > best.1 {
                    best = (class, acc);
                }
            }
            best.0
        }
    }

    fn build_neural_features(
        coeffs: &[i16; BLOCK_PIXELS],
        energy: u32,
        bundle_width: u8,
        quantizer: u8,
        frame_type: FrameType,
        block_index: usize,
    ) -> [i8; NEURAL_FEATURES] {
        let mut features = [0i8; NEURAL_FEATURES];
        features[0] = energy_bucket(energy) as i8;
        features[1] = max_zero_run_in_block(coeffs) as i8;
        features[2] = quantizer_to_bucket(quantizer) as i8;
        features[3] = bundle_width.clamp(1, MAX_BUNDLE_WIDTH as u8) as i8;
        let non_zero = coeffs.iter().skip(1).filter(|&&c| c != 0).count();
        features[4] = non_zero.min(i8::MAX as usize) as i8;
        features[5] = ((block_index as u8) & 0x0F) as i8;
        features[6] = if matches!(frame_type, FrameType::Intra) {
            0
        } else {
            1
        };
        let tail_energy: u32 = coeffs
            .iter()
            .skip(BLOCK_PIXELS.saturating_sub(8))
            .map(|&c| c.unsigned_abs())
            .map(u32::from)
            .sum();
        features[7] = tail_energy.min(127) as i8;
        features
    }

    #[cfg(test)]
    mod rdo_tests {
        use super::*;

        #[test]
        fn dp_optimizer_reduces_rate_on_sparse_block() {
            let mut coeffs = [0i16; BLOCK_PIXELS];
            coeffs[0] = 10;
            coeffs[1] = 2;
            coeffs[5] = -1;
            coeffs[12] = 1;
            let before_bits = estimate_rle_bits(&coeffs);
            let mut working = coeffs;
            let report = optimize_block_dp(
                &mut working,
                RdoMode::DynamicProgramming,
                1.5,
                None,
                3,
                10,
                FrameType::Intra,
                0,
            );
            assert!(
                report.after_bits <= before_bits,
                "RDO must not increase rate"
            );
            assert!(
                report.distortion_penalty > 0,
                "zeroing must record distortion"
            );
            // ensure DC preserved
            assert_eq!(working[0], coeffs[0]);
        }

        #[test]
        fn neural_predictor_is_deterministic() {
            let predictor = NeuralPredictor::from_seed(DEFAULT_NEURAL_SEED);
            let coeffs = [0i16; BLOCK_PIXELS];
            let features = build_neural_features(&coeffs, 0, 2, 8, FrameType::Intra, 1);
            let first = predictor.predict(&features);
            let second = predictor.predict(&features);
            assert_eq!(first, second);
        }

        #[test]
        fn context_frequency_records_symbols() {
            let dims = FrameDimensions::new(8, 8);
            let tables = default_bundle_tables();
            let mut recorder =
                BundleStreamRecorder::new(2, dims, 4, tables, BundleAcceleration::None, 0, None);
            recorder.record_bundle(
                BundleType::SignAndMagnitude,
                BundleContextId::new(7),
                0b11,
                2,
                BundleFlushReason::TypeComplete,
            );
            let telemetry = recorder.finish();
            let entry = telemetry
                .context_frequencies
                .iter()
                .find(|entry| entry.context == BundleContextId::new(7))
                .expect("context frequency recorded");
            assert_eq!(entry.bundles, 1);
            assert_eq!(entry.symbol_counts[3], 1);
        }

        #[test]
        fn context_remap_loader_maps_and_escapes() {
            let path = std::env::temp_dir().join("remap_loader.json");
            let json = r#"
            {
                "escape_context": 65535,
                "kept": [{ "original": 7, "remapped": 1 }],
                "dropped": [{ "original": 9 }]
            }"#;
            std::fs::write(&path, json).expect("write remap json");
            let remap =
                load_bundle_context_remap_from_json(&path).expect("remap should parse correctly");
            assert_eq!(remap.map(BundleContextId::new(7)), BundleContextId::new(1));
            assert_eq!(
                remap.map(BundleContextId::new(9)),
                BundleContextId::new(u16::MAX)
            );
            assert_eq!(remap.remapped_count(), 1);
            assert_eq!(remap.dropped_count(), 1);
            let _ = std::fs::remove_file(&path);
        }
    }

    struct BundleStreamRecorder {
        stats: BundledStats,
        tokens: Vec<BundledToken>,
        bundles: Vec<BundleRecord>,
        context_stats: BTreeMap<BundleContextId, BundleContextStats>,
        context_frequency: BTreeMap<BundleContextId, ContextFrequency>,
        width: u8,
        blocks_per_row: usize,
        quantizer_bucket: u8,
        tables: Arc<BundleAnsTables>,
        acceleration: BundleAcceleration,
        prefetch_distance: u16,
        pending_blocks: VecDeque<BlockBundleState>,
        current_block: Option<BlockBundleState>,
        last_context: Option<BundleContextId>,
        context_remap: Option<Arc<BundleContextRemap>>,
    }

    impl BundleStreamRecorder {
        fn new(
            bundle_width: u8,
            dims: FrameDimensions,
            quantizer: u8,
            tables: Arc<BundleAnsTables>,
            acceleration: BundleAcceleration,
            prefetch_distance: u16,
            context_remap: Option<Arc<BundleContextRemap>>,
        ) -> Self {
            Self {
                stats: BundledStats {
                    bundle_width,
                    ..BundledStats::default()
                },
                tokens: Vec::new(),
                bundles: Vec::new(),
                context_stats: BTreeMap::new(),
                context_frequency: BTreeMap::new(),
                width: bundle_width.clamp(1, 4),
                blocks_per_row: (dims.width as usize / BLOCK_SIZE).max(1),
                quantizer_bucket: quantizer_to_bucket(quantizer),
                tables,
                acceleration,
                prefetch_distance,
                pending_blocks: VecDeque::new(),
                current_block: None,
                last_context: None,
                context_remap,
            }
        }

        fn context_stats_entry(&mut self, context: BundleContextId) -> &mut BundleContextStats {
            self.context_stats
                .entry(context)
                .or_insert_with(|| BundleContextStats::new(context))
        }

        fn map_context(&self, context: BundleContextId) -> BundleContextId {
            if let Some(remap) = self.context_remap.as_ref() {
                remap.map(context)
            } else {
                context
            }
        }

        fn finish(self) -> BundledTelemetry {
            let (ans_stream, used_acceleration) = encode_bundle_stream_with_opts(
                self.tables.as_ref(),
                &self.bundles,
                self.acceleration,
                self.prefetch_distance,
            );
            let remap = self.context_remap.as_ref().map(|map| ContextRemapSummary {
                escape_context: map.escape_context(),
                remapped: map.remapped_count(),
                dropped: map.dropped_count(),
            });
            BundledTelemetry {
                stats: self.stats,
                tokens: self.tokens,
                bundles: self.bundles,
                context_frequencies: self.context_frequency.into_values().collect(),
                ans_stream,
                ans_precision_bits: self.tables.precision_bits(),
                tables_checksum: self.tables.checksum(),
                context_stats: self.context_stats.into_values().collect(),
                acceleration: used_acceleration,
                prefetch_distance: self.prefetch_distance,
                context_remap: remap,
                rdo: None,
            }
        }

        fn record_bundle(
            &mut self,
            bundle_type: BundleType,
            context: BundleContextId,
            bits: u8,
            bit_len: u8,
            flush: BundleFlushReason,
        ) {
            if let Some(previous_context) = self.last_context
                && previous_context != context
            {
                self.stats.record_flush(BundleFlushReason::ContextSwitch);
                self.context_stats_entry(previous_context)
                    .record_flush(BundleFlushReason::ContextSwitch);
            }
            self.last_context = Some(context);
            self.stats.bundles_total = self.stats.bundles_total.saturating_add(1);
            self.stats.record_flush(flush);
            if matches!(bundle_type, BundleType::SignificanceRle) {
                self.stats.significance_rle = self.stats.significance_rle.saturating_add(1);
            }
            self.context_stats_entry(context)
                .record_bundle(bundle_type, flush);
            let freq = self
                .context_frequency
                .entry(context)
                .or_insert_with(|| ContextFrequency {
                    context,
                    ..ContextFrequency::default()
                });
            freq.bundles = freq.bundles.saturating_add(1);
            freq.total_bits = freq.total_bits.saturating_add(u64::from(bit_len));
            let symbol_idx = usize::from(bits) & (RDO_MAX_SYMBOLS - 1);
            freq.symbol_counts[symbol_idx] = freq.symbol_counts[symbol_idx].saturating_add(1);
            self.bundles.push(BundleRecord {
                bundle_type,
                context,
                bits,
                bit_len,
                flush,
            });
        }

        fn accumulate_stats(&mut self, value: i16) {
            let nonzero = value != 0;
            if nonzero {
                self.stats.significance_one = self.stats.significance_one.saturating_add(1);
            } else {
                self.stats.significance_zero = self.stats.significance_zero.saturating_add(1);
            }

            let negative = value < 0;
            if negative {
                self.stats.sign_negative = self.stats.sign_negative.saturating_add(1);
            } else {
                self.stats.sign_positive = self.stats.sign_positive.saturating_add(1);
            }

            if self.width >= 3 && value != 0 {
                let parity = (value.wrapping_abs() as u32 & 1) as u8;
                if parity == 1 {
                    self.stats.parity_one = self.stats.parity_one.saturating_add(1);
                }
            }

            if self.width >= 4 && value.abs() >= 2 {
                self.stats.geq2_one = self.stats.geq2_one.saturating_add(1);
            }
        }

        fn record_block(
            &mut self,
            block_index: usize,
            coeffs: &[i16; BLOCK_PIXELS],
            frame_type: FrameType,
        ) {
            let mut prev_level_abs = 0u8;
            let mut slots = Vec::with_capacity(BLOCK_PIXELS - 1);
            for (order, &slot) in ZIG_ZAG.iter().enumerate().skip(1) {
                let coeff = coeffs[slot];
                self.accumulate_stats(coeff);
                let (bundle_type, _, _) = bundle_bits(coeff, self.width);
                let context = self.map_context(self.compute_context(
                    bundle_type,
                    block_index,
                    order,
                    slot,
                    coeff,
                    prev_level_abs,
                    frame_type,
                    coeffs,
                ));
                slots.push(SlotContext {
                    context,
                    is_non_zero: coeff != 0,
                });
                prev_level_abs = (coeff.saturating_abs() as u16).min(0xFF) as u8;
            }
            self.pending_blocks
                .push_back(BlockBundleState::new(block_index, slots));
        }

        fn ensure_current_block(&mut self) {
            if self.current_block.is_none()
                && let Some(next) = self.pending_blocks.pop_front()
            {
                self.current_block = Some(next);
            }
        }

        fn peek_slot(&mut self) -> Option<SlotContext> {
            self.ensure_current_block();
            self.current_block
                .as_ref()
                .and_then(BlockBundleState::peek_slot)
        }

        fn pop_slot(&mut self) -> Option<SlotContext> {
            loop {
                self.ensure_current_block();
                match self.current_block.as_mut() {
                    Some(block) => {
                        if let Some(slot) = block.next_slot() {
                            return Some(slot);
                        }
                        self.current_block = None;
                    }
                    None => return None,
                }
            }
        }

        fn flush_pending_zero_slots(&mut self) {
            while let Some(slot) = self.pop_slot() {
                debug_assert!(!slot.is_non_zero);
                let mut run = 1u16;
                while let Some(next) = self.peek_slot() {
                    if next.is_non_zero || next.context != slot.context {
                        break;
                    }
                    self.pop_slot()
                        .expect("peeked slot should still be available");
                    run = run.saturating_add(1);
                }
                self.emit_zero_run_bundle(slot.context, run);
            }
        }

        fn record_zero_run(&mut self, zeros: u8) {
            let mut remaining = zeros;
            while remaining > 0 {
                let slot = self
                    .pop_slot()
                    .expect("zero slot must exist before nonzero run");
                debug_assert!(!slot.is_non_zero);
                let context = slot.context;
                let mut run: u16 = 1;
                while run < u16::from(remaining) {
                    if let Some(next) = self.peek_slot() {
                        if next.is_non_zero || next.context != context {
                            break;
                        }
                        self.pop_slot()
                            .expect("peeked slot should still be available");
                        run = run.saturating_add(1);
                        continue;
                    }
                    break;
                }
                self.emit_zero_run_bundle(context, run);
                remaining = remaining.saturating_sub(run as u8);
            }
        }

        fn emit_zero_run_bundle(&mut self, context: BundleContextId, mut run: u16) {
            while run > 0 {
                let (bits, bit_len, consumed) = bundle_zero_run_symbol(run, self.width);
                self.record_bundle(
                    BundleType::SignificanceRle,
                    context,
                    bits,
                    bit_len,
                    BundleFlushReason::TypeComplete,
                );
                run = run.saturating_sub(consumed);
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn compute_context(
            &self,
            bundle_type: BundleType,
            block_index: usize,
            zigzag_order: usize,
            coeff_slot: usize,
            coeff: i16,
            prev_level_abs: u8,
            frame_type: FrameType,
            coeffs: &[i16; BLOCK_PIXELS],
        ) -> BundleContextId {
            let block_x = block_index % self.blocks_per_row;
            let block_y = block_index / self.blocks_per_row;
            let block_tile = (((block_x as u32) & 0x7) | (((block_y as u32) & 0x7) << 3)) as u8;
            let neighbor_nz = neighbor_nonzero_count(coeffs, coeff_slot);
            let subband = subband_class(coeff_slot);
            let pos_class = position_class(zigzag_order);
            let prev_bucket = level_bucket(prev_level_abs);
            let frame_class: u8 = if frame_type == FrameType::Intra { 0 } else { 1 };
            let coeff_bucket = (coeff != 0) as u8;

            let mut hash = 0x811C9DC5u32;
            hash = hash_mix(hash, bundle_type.as_u8() as u32);
            hash = hash_mix(hash, subband.into());
            hash = hash_mix(hash, pos_class.into());
            hash = hash_mix(hash, neighbor_nz.into());
            hash = hash_mix(hash, prev_bucket.into());
            hash = hash_mix(hash, self.quantizer_bucket.into());
            hash = hash_mix(hash, frame_class.into());
            hash = hash_mix(hash, block_tile.into());
            hash = hash_mix(hash, coeff_bucket.into());
            hash = hash_mix(hash, (zigzag_order as u32) & 0x3F);
            BundleContextId::new((hash & 0xFFFF) as u16)
        }
    }

    impl BundledHooks for BundleStreamRecorder {
        fn record_block_coeffs(
            &mut self,
            coeffs: &[i16; BLOCK_PIXELS],
            block_index: usize,
            frame_type: FrameType,
        ) {
            self.record_block(block_index, coeffs, frame_type);
        }

        fn record_dc(&mut self, diff: i16) {
            self.ensure_current_block();
            self.stats.dc_events = self.stats.dc_events.saturating_add(1);
            self.tokens.push(BundledToken::DcDiff(diff));
        }

        fn record_ac(&mut self, zeros: u8, value: i16) {
            self.stats.ac_events = self.stats.ac_events.saturating_add(1);
            self.stats.zero_run_total = self.stats.zero_run_total.saturating_add(u64::from(zeros));
            self.stats.max_zero_run = self.stats.max_zero_run.max(zeros);
            if value != 0 {
                self.stats.nonzero_levels = self.stats.nonzero_levels.saturating_add(1);
            }

            self.record_zero_run(zeros);
            let slot = self
                .pop_slot()
                .expect("non-zero slot must exist after zero run");
            debug_assert!(slot.is_non_zero);
            let (bundle_type, bits, len) = bundle_bits(value, self.width);
            self.record_bundle(
                bundle_type,
                slot.context,
                bits,
                len,
                BundleFlushReason::TypeComplete,
            );

            self.tokens.push(BundledToken::Ac { run: zeros, value });
        }

        fn record_eob(&mut self) {
            self.flush_pending_zero_slots();
            self.stats.blocks_encoded = self.stats.blocks_encoded.saturating_add(1);
            if let Some(last_context) = self.bundles.last().map(|record| record.context) {
                self.context_stats_entry(last_context)
                    .record_flush(BundleFlushReason::EndOfBlock);
            }
            self.stats.record_flush(BundleFlushReason::EndOfBlock);
            self.tokens.push(BundledToken::EndOfBlock);
            self.current_block = None;
            self.last_context = None;
        }
    }

    #[derive(Clone, Copy)]
    struct SlotContext {
        context: BundleContextId,
        is_non_zero: bool,
    }

    struct BlockBundleState {
        #[allow(dead_code)]
        block_index: usize,
        slots: Vec<SlotContext>,
        cursor: usize,
    }

    impl BlockBundleState {
        fn new(block_index: usize, slots: Vec<SlotContext>) -> Self {
            Self {
                block_index,
                slots,
                cursor: 0,
            }
        }

        fn next_slot(&mut self) -> Option<SlotContext> {
            let slot = self.slots.get(self.cursor).copied();
            if slot.is_some() {
                self.cursor = self.cursor.saturating_add(1);
            }
            slot
        }

        fn peek_slot(&self) -> Option<SlotContext> {
            self.slots.get(self.cursor).copied()
        }
    }

    const BUNDLE_ANS_PRECISION_BITS: u8 = 12;
    const BUNDLE_RANS_BYTE_L: u32 = 1 << 23;

    #[derive(Clone, Debug)]
    struct SymbolTable {
        freq: Vec<u16>,
        start: Vec<u16>,
        precision_bits: u8,
        decode: Vec<AnsSymbol>,
    }

    #[derive(Clone, Copy, Debug)]
    struct AnsSymbol {
        symbol: u16,
        start: u16,
        freq: u16,
    }

    pub(super) const MAX_BUNDLE_WIDTH: usize = 4;

    impl SymbolTable {
        fn from_group(group: &RansGroupTableV1) -> Result<Self, BundleTableError> {
            let width_bits = match group.width_bits {
                0 => infer_group_width_bits(group.group_size)?,
                bits => bits,
            };
            let expected = 1usize << width_bits;
            if expected == 0 || group.frequencies.len() != expected {
                return Err(BundleTableError::InvalidGroup {
                    bit_len: 0,
                    reason: "group frequencies length mismatch",
                });
            }
            if group.cumulative.len() != expected + 1 {
                return Err(BundleTableError::InvalidGroup {
                    bit_len: 0,
                    reason: "group cumulative length mismatch",
                });
            }
            if usize::from(group.group_size) != expected {
                return Err(BundleTableError::InvalidGroup {
                    bit_len: 0,
                    reason: "group size does not match width bits",
                });
            }
            let total = 1u32.checked_shl(group.precision_bits.into()).ok_or(
                BundleTableError::InvalidGroup {
                    bit_len: 0,
                    reason: "precision bits overflow",
                },
            )?;
            if *group.cumulative.first().unwrap_or(&1) != 0
                || *group.cumulative.last().unwrap_or(&0) != total
            {
                return Err(BundleTableError::InvalidGroup {
                    bit_len: 0,
                    reason: "cumulative range mismatch",
                });
            }
            let freq_sum: u32 = group
                .frequencies
                .iter()
                .map(|&value| u32::from(value))
                .sum();
            if freq_sum != total {
                return Err(BundleTableError::InvalidGroup {
                    bit_len: 0,
                    reason: "frequency sum mismatch",
                });
            }
            let mut start = Vec::with_capacity(expected);
            for &value in group.cumulative.iter().take(expected) {
                start.push(
                    u16::try_from(value).map_err(|_| BundleTableError::InvalidGroup {
                        bit_len: 0,
                        reason: "cumulative value exceeds u16 range",
                    })?,
                );
            }
            let table = SymbolTable::from_frequencies(
                group.frequencies.clone(),
                start,
                group.precision_bits,
            );
            Ok(table)
        }

        fn from_frequencies(freq: Vec<u16>, start: Vec<u16>, precision_bits: u8) -> Self {
            let decode = build_decode_table(&freq, &start, precision_bits);
            Self {
                freq,
                start,
                precision_bits,
                decode,
            }
        }
    }

    fn infer_group_width_bits(group_size: u16) -> Result<u8, BundleTableError> {
        if group_size == 0 {
            return Err(BundleTableError::InvalidGroup {
                bit_len: 0,
                reason: "group size must be > 0",
            });
        }
        let value = u32::from(group_size);
        if value & (value - 1) != 0 {
            return Err(BundleTableError::InvalidGroup {
                bit_len: 0,
                reason: "group size must be a power of two",
            });
        }
        Ok(value.trailing_zeros() as u8)
    }

    fn build_uniform_symbol_table(bit_len: u8, precision_bits: u8) -> SymbolTable {
        let alphabet = 1usize << bit_len.max(1);
        let precision = 1usize << precision_bits;
        let base_freq = (precision / alphabet) as u16;
        let mut freq = vec![base_freq; alphabet];
        let mut remainder = precision - base_freq as usize * alphabet;
        let mut idx = 0usize;
        while remainder > 0 {
            freq[idx] = freq[idx].saturating_add(1);
            remainder -= 1;
            idx += 1;
            if idx == alphabet {
                idx = 0;
            }
        }
        let mut start = Vec::with_capacity(alphabet);
        let mut cumulative = 0u32;
        for &value in &freq {
            start.push(cumulative as u16);
            cumulative += u32::from(value);
        }
        SymbolTable::from_frequencies(freq, start, precision_bits)
    }

    fn derive_significance_table(source: &SymbolTable) -> SymbolTable {
        let mut freq = vec![0u16; 2];
        for (symbol, &value) in source.freq.iter().enumerate() {
            let sig = symbol & 1;
            freq[sig] = freq[sig].saturating_add(value);
        }
        let mut start = Vec::with_capacity(2);
        let mut cumulative = 0u32;
        for &value in &freq {
            start.push(cumulative as u16);
            cumulative += u32::from(value);
        }
        SymbolTable::from_frequencies(freq, start, source.precision_bits)
    }

    fn build_decode_table(freq: &[u16], start: &[u16], precision_bits: u8) -> Vec<AnsSymbol> {
        let precision = 1usize << precision_bits;
        let mut table = vec![
            AnsSymbol {
                symbol: 0,
                start: 0,
                freq: 1,
            };
            precision
        ];
        for (symbol, (&begin, &frequency)) in start.iter().zip(freq.iter()).enumerate() {
            table
                .iter_mut()
                .skip(begin as usize)
                .take(frequency as usize)
                .for_each(|slot| {
                    *slot = AnsSymbol {
                        symbol: symbol as u16,
                        start: begin,
                        freq: frequency,
                    };
                });
        }
        table
    }

    #[inline]
    fn prefetch_bundle_record(record: &BundleRecord) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            use core::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};
            _mm_prefetch(record as *const _ as *const i8, _MM_HINT_T0);
        }

        #[cfg(target_arch = "aarch64")]
        unsafe {
            core::arch::asm!(
                "prfm pldl1keep, [{0}]",
                in(reg) record,
                options(readonly, nostack, preserves_flags)
            );
        }
    }

    struct BundleRansEncoder<'a> {
        tables: &'a BundleAnsTables,
        precision_bits: u8,
        state: u32,
        buffer: Vec<u8>,
        table_cache: [Option<&'a SymbolTable>; MAX_BUNDLE_WIDTH + 1],
    }

    impl<'a> BundleRansEncoder<'a> {
        fn new(tables: &'a BundleAnsTables) -> Self {
            let mut table_cache: [Option<&'a SymbolTable>; MAX_BUNDLE_WIDTH + 1] =
                core::array::from_fn(|_| None);
            for bit_len in 1..=tables.max_width() {
                table_cache[bit_len as usize] = Some(tables.table_for_bits(bit_len));
            }
            Self {
                tables,
                precision_bits: tables.precision_bits(),
                state: BUNDLE_RANS_BYTE_L,
                buffer: Vec::new(),
                table_cache,
            }
        }

        fn encode_record(&mut self, record: &BundleRecord) {
            let table = self
                .table_cache
                .get(record.bit_len as usize)
                .and_then(Option::as_ref)
                .copied()
                .unwrap_or_else(|| self.tables.table_for_bits(record.bit_len));
            let symbol = (record.bits & ((1 << record.bit_len) - 1)) as usize;
            self.encode_symbol(symbol, table);
        }

        fn encode_symbol(&mut self, symbol: usize, table: &SymbolTable) {
            let freq = table.freq[symbol] as u32;
            let start = table.start[symbol] as u32;
            while self.state >= (freq << (32 - self.precision_bits)) {
                self.buffer.push((self.state & 0xFF) as u8);
                self.state >>= 8;
            }
            self.state = ((self.state / freq) << self.precision_bits) + (self.state % freq) + start;
        }

        fn finish(mut self) -> Vec<u8> {
            for _ in 0..4 {
                self.buffer.push((self.state & 0xFF) as u8);
                self.state >>= 8;
            }
            self.buffer
        }
    }

    struct BundleRansDecoder<'a> {
        tables: &'a BundleAnsTables,
        precision_bits: u8,
        buffer: &'a [u8],
        cursor: usize,
        state: u32,
    }

    impl<'a> BundleRansDecoder<'a> {
        fn new(buffer: &'a [u8], tables: &'a BundleAnsTables) -> Result<Self, BundleDecodeError> {
            if buffer.len() < 4 {
                return Err(BundleDecodeError::TruncatedState);
            }
            let len = buffer.len();
            let mut state = 0u32;
            for shift in 0..4 {
                state |= u32::from(buffer[len - 4 + shift]) << (shift * 8);
            }
            let cursor = len - 4;
            Ok(Self {
                tables,
                precision_bits: tables.precision_bits(),
                buffer,
                cursor,
                state,
            })
        }

        fn decode_record(&mut self, record: &BundleRecord) -> Result<u8, BundleDecodeError> {
            let table = self.tables.table_for_bits(record.bit_len);
            self.decode_symbol(table).map(|sym| sym as u8)
        }

        fn decode_symbol(&mut self, table: &SymbolTable) -> Result<u16, BundleDecodeError> {
            let mask = (1u32 << self.precision_bits) - 1;
            let idx = (self.state & mask) as usize;
            let sym = table.decode[idx];
            let scaled = self.state >> self.precision_bits;
            self.state = sym.freq as u32 * scaled + (idx as u32 - sym.start as u32);
            self.renormalize()?;
            Ok(sym.symbol)
        }

        fn renormalize(&mut self) -> Result<(), BundleDecodeError> {
            while self.state < BUNDLE_RANS_BYTE_L && self.cursor > 0 {
                self.cursor -= 1;
                self.state = (self.state << 8) | u32::from(self.buffer[self.cursor]);
            }
            if self.state < BUNDLE_RANS_BYTE_L && self.cursor == 0 {
                return Err(BundleDecodeError::RenormalizeUnderflow);
            }
            Ok(())
        }
    }

    const SIMD_BUNDLE_MAGIC: [u8; 4] = *b"BR4\x01";

    #[cfg(test)]
    fn encode_bundle_stream(tables: &BundleAnsTables, bundles: &[BundleRecord]) -> Vec<u8> {
        let (stream, _acceleration) =
            encode_bundle_stream_with_opts(tables, bundles, BundleAcceleration::None, 0);
        stream
    }

    fn encode_bundle_stream_with_opts(
        tables: &BundleAnsTables,
        bundles: &[BundleRecord],
        requested: BundleAcceleration,
        prefetch_distance: u16,
    ) -> (Vec<u8>, BundleAcceleration) {
        let has_rle = bundles
            .iter()
            .any(|record| matches!(record.bundle_type, BundleType::SignificanceRle));
        if has_rle {
            let mut stream = Vec::with_capacity(bundles.len());
            for record in bundles {
                stream.push(record.bits & ((1u8 << record.bit_len) - 1));
            }
            // RLE bundles are scalar-only until SIMD tables are calibrated.
            return (stream, BundleAcceleration::None);
        }
        if requested == BundleAcceleration::CpuSimd && cpu_simd_supported() {
            let stream = encode_bundle_stream_simd(tables, bundles, prefetch_distance);
            let acceleration = if stream.starts_with(&SIMD_BUNDLE_MAGIC) {
                BundleAcceleration::CpuSimd
            } else {
                BundleAcceleration::None
            };
            (stream, acceleration)
        } else {
            (
                encode_bundle_stream_scalar(tables, bundles, prefetch_distance),
                BundleAcceleration::None,
            )
        }
    }

    fn encode_bundle_stream_scalar(
        tables: &BundleAnsTables,
        bundles: &[BundleRecord],
        prefetch_distance: u16,
    ) -> Vec<u8> {
        let mut encoder = BundleRansEncoder::new(tables);
        let distance = prefetch_distance as usize;
        for rev_idx in 0..bundles.len() {
            let idx = bundles.len() - 1 - rev_idx;
            if distance > 0
                && let Some(prefetch_idx) = idx.checked_sub(distance)
            {
                prefetch_bundle_record(&bundles[prefetch_idx]);
            }
            encoder.encode_record(&bundles[idx]);
        }
        encoder.finish()
    }

    fn encode_bundle_stream_simd(
        tables: &BundleAnsTables,
        bundles: &[BundleRecord],
        prefetch_distance: u16,
    ) -> Vec<u8> {
        let mut states = [BUNDLE_RANS_BYTE_L; 4];
        let mut buffers: [Vec<u8>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let distance = prefetch_distance as usize;
        let precision_bits = tables.precision_bits();
        let mut table_cache: [Option<&SymbolTable>; MAX_BUNDLE_WIDTH + 1] =
            core::array::from_fn(|_| None);
        for bit_len in 1..=tables.max_width() {
            table_cache[bit_len as usize] = Some(tables.table_for_bits(bit_len));
        }

        for rev_idx in 0..bundles.len() {
            let idx = bundles.len() - 1 - rev_idx;
            if distance > 0
                && let Some(prefetch_idx) = idx.checked_sub(distance)
            {
                prefetch_bundle_record(&bundles[prefetch_idx]);
            }
            let lane = rev_idx & 3;
            let record = &bundles[idx];
            let table = table_cache[record.bit_len as usize]
                .unwrap_or_else(|| tables.table_for_bits(record.bit_len));
            let symbol = (record.bits & ((1 << record.bit_len) - 1)) as usize;
            let state = &mut states[lane];
            let buffer = &mut buffers[lane];
            let freq = table.freq[symbol] as u32;
            let start = table.start[symbol] as u32;
            while *state >= (freq << (32 - precision_bits)) {
                buffer.push((*state & 0xFF) as u8);
                *state >>= 8;
            }
            *state = ((*state / freq) << precision_bits) + (*state % freq) + start;
        }

        for (state, buffer) in states.iter_mut().zip(buffers.iter_mut()) {
            for _ in 0..4 {
                buffer.push((*state & 0xFF) as u8);
                *state >>= 8;
            }
        }

        let mut payload_len = SIMD_BUNDLE_MAGIC.len();
        let mut lane_lengths = [0usize; 4];
        let mut overflowed = false;
        for (idx, buf) in buffers.iter().enumerate() {
            lane_lengths[idx] = buf.len();
            if lane_lengths[idx] > u32::MAX as usize {
                overflowed = true;
            }
            payload_len = payload_len.saturating_add(4).saturating_add(buf.len());
        }
        if overflowed {
            // SIMD header stores lane lengths as u32, so fall back to scalar on overflow.
            return encode_bundle_stream_scalar(tables, bundles, prefetch_distance);
        }

        let mut out = Vec::with_capacity(payload_len);
        out.extend_from_slice(&SIMD_BUNDLE_MAGIC);
        for len in lane_lengths {
            let len_u32 = len as u32;
            out.extend_from_slice(&len_u32.to_le_bytes());
        }
        for buf in buffers {
            out.extend_from_slice(&buf);
        }
        out
    }

    fn cpu_simd_supported() -> bool {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            return std::arch::is_x86_feature_detected!("avx2");
        }
        #[cfg(target_arch = "aarch64")]
        {
            return std::arch::is_aarch64_feature_detected!("neon");
        }
        #[allow(unreachable_code)]
        false
    }

    /// Decode a bundled rANS stream back into the raw bundle symbols.
    pub fn decode_bundle_stream(
        stream: &[u8],
        bundles: &[BundleRecord],
        tables: &BundleAnsTables,
    ) -> Result<Vec<u8>, BundleDecodeError> {
        if bundles
            .iter()
            .any(|record| matches!(record.bundle_type, BundleType::SignificanceRle))
        {
            return Ok(bundles
                .iter()
                .map(|record| record.bits & ((1u8 << record.bit_len) - 1))
                .collect());
        }
        if stream.starts_with(&SIMD_BUNDLE_MAGIC) {
            return decode_bundle_stream_simd(stream, bundles, tables);
        }
        let mut decoder = BundleRansDecoder::new(stream, tables)?;
        let mut out = Vec::with_capacity(bundles.len());
        for record in bundles {
            out.push(decoder.decode_record(record)?);
        }
        Ok(out)
    }

    fn decode_bundle_stream_simd(
        stream: &[u8],
        bundles: &[BundleRecord],
        tables: &BundleAnsTables,
    ) -> Result<Vec<u8>, BundleDecodeError> {
        let header_len = SIMD_BUNDLE_MAGIC.len();
        if stream.len() < header_len + 16 {
            return Err(BundleDecodeError::InvalidSimdHeader);
        }
        let mut cursor = header_len;
        let mut lengths = [0usize; 4];
        for slot in lengths.iter_mut() {
            let mut buf = [0u8; 4];
            if cursor + 4 > stream.len() {
                return Err(BundleDecodeError::InvalidSimdHeader);
            }
            buf.copy_from_slice(&stream[cursor..cursor + 4]);
            *slot = u32::from_le_bytes(buf) as usize;
            cursor += 4;
        }
        let total_len = lengths
            .iter()
            .try_fold(0usize, |acc, len| acc.checked_add(*len))
            .ok_or(BundleDecodeError::LengthMismatch)?;
        let expected_end = cursor
            .checked_add(total_len)
            .ok_or(BundleDecodeError::LengthMismatch)?;
        if stream.len() != expected_end {
            return Err(BundleDecodeError::LengthMismatch);
        }
        let mut decoders: [Option<BundleRansDecoder<'_>>; 4] = [None, None, None, None];
        let mut lane_cursor = cursor;
        for (idx, len) in lengths.iter().enumerate() {
            let next = lane_cursor
                .checked_add(*len)
                .ok_or(BundleDecodeError::LengthMismatch)?;
            let lane_slice = stream
                .get(lane_cursor..next)
                .ok_or(BundleDecodeError::LengthMismatch)?;
            decoders[idx] = Some(BundleRansDecoder::new(lane_slice, tables)?);
            lane_cursor = next;
        }

        let mut out = Vec::with_capacity(bundles.len());
        for (idx, record) in bundles.iter().enumerate() {
            let lane = (bundles.len() - 1 - idx) & 3;
            let decoder = decoders[lane]
                .as_mut()
                .ok_or(BundleDecodeError::InvalidSimdHeader)?;
            out.push(decoder.decode_record(record)?);
        }
        Ok(out)
    }

    fn bundle_bits(value: i16, width: u8) -> (BundleType, u8, u8) {
        let nonzero = value != 0;
        let mut bits = (nonzero as u8) & 0x1;
        let mut len = 1u8;
        if !nonzero || width <= 1 {
            return (BundleType::SignificanceOnly, bits, len);
        }

        let negative = value < 0;
        bits = (bits << 1) | (negative as u8);
        len += 1;
        if width <= 2 {
            return (BundleType::SignAndMagnitude, bits, len);
        }

        let parity = (value.saturating_abs() as u16 & 1) as u8;
        bits = (bits << 1) | parity;
        len += 1;
        if width <= 3 {
            return (BundleType::SignParity, bits, len);
        }

        let geq2 = (value.saturating_abs() >= 2) as u8;
        bits = (bits << 1) | geq2;
        len += 1;
        (BundleType::SignParityLevel, bits, len)
    }

    fn bundle_zero_run_symbol(run: u16, width: u8) -> (u8, u8, u16) {
        let bit_len = width.max(2).min(MAX_BUNDLE_WIDTH as u8);
        let max_symbol = (1u16 << bit_len) - 1;
        let consumed = run.min(max_symbol);
        let bits = u8::try_from(consumed).unwrap_or(u8::MAX);
        (bits, bit_len, consumed)
    }

    fn quantizer_to_bucket(qp: u8) -> u8 {
        match qp {
            0..=12 => 0,
            13..=24 => 1,
            25..=36 => 2,
            _ => 3,
        }
    }

    fn position_class(order: usize) -> u8 {
        match order {
            0 => 0,
            1..=4 => 1,
            5..=15 => 2,
            16..=31 => 3,
            _ => 4,
        }
    }

    fn subband_class(slot: usize) -> u8 {
        let row = slot / BLOCK_SIZE;
        let col = slot % BLOCK_SIZE;
        let low_cut = 3;
        match (row <= low_cut, col <= low_cut) {
            (true, true) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (false, false) => 3,
        }
    }

    fn neighbor_nonzero_count(coeffs: &[i16; BLOCK_PIXELS], slot: usize) -> u8 {
        let row = slot / BLOCK_SIZE;
        let col = slot % BLOCK_SIZE;
        let mut count = 0u8;
        if col > 0 && coeffs[row * BLOCK_SIZE + (col - 1)] != 0 {
            count = count.saturating_add(1);
        }
        if row > 0 && coeffs[(row - 1) * BLOCK_SIZE + col] != 0 {
            count = count.saturating_add(1);
        }
        if row > 0 && col > 0 && coeffs[(row - 1) * BLOCK_SIZE + (col - 1)] != 0 {
            count = count.saturating_add(1);
        }
        if row > 0 && col + 1 < BLOCK_SIZE && coeffs[(row - 1) * BLOCK_SIZE + (col + 1)] != 0 {
            count = count.saturating_add(1);
        }
        count.min(3)
    }

    fn level_bucket(value: u8) -> u8 {
        match value {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 3,
        }
    }

    fn hash_mix(acc: u32, value: u32) -> u32 {
        acc.wrapping_mul(16777619) ^ value
    }

    pub(crate) fn encode_block_rle(
        coeffs: &[i16; BLOCK_PIXELS],
        prev_dc: &mut i16,
        out: &mut Vec<u8>,
        hooks: &mut dyn BundledHooks,
        block_index: usize,
        frame_type: FrameType,
    ) {
        hooks.record_block_coeffs(coeffs, block_index, frame_type);
        let dc = coeffs[0];
        let diff = dc.wrapping_sub(*prev_dc);
        out.extend_from_slice(&diff.to_le_bytes());
        hooks.record_dc(diff);
        *prev_dc = dc;

        let mut pos = 1usize;
        while pos < BLOCK_PIXELS {
            let mut zero_run = 0usize;
            while pos < BLOCK_PIXELS && coeffs[ZIG_ZAG[pos]] == 0 {
                zero_run += 1;
                pos += 1;
            }

            if pos == BLOCK_PIXELS {
                out.push(RLE_EOB);
                out.extend_from_slice(&0i16.to_le_bytes());
                hooks.record_eob();
                return;
            }

            while zero_run > MAX_ZERO_RUN {
                out.push(MAX_ZERO_RUN as u8);
                out.extend_from_slice(&0i16.to_le_bytes());
                hooks.record_ac(MAX_ZERO_RUN as u8, 0);
                zero_run -= MAX_ZERO_RUN;
            }

            out.push(zero_run as u8);
            out.extend_from_slice(&coeffs[ZIG_ZAG[pos]].to_le_bytes());
            hooks.record_ac(zero_run as u8, coeffs[ZIG_ZAG[pos]]);
            pos += 1;
        }

        out.push(RLE_EOB);
        out.extend_from_slice(&0i16.to_le_bytes());
        hooks.record_eob();
    }

    pub(crate) fn decode_block_rle(
        bytes: &[u8],
        offset: &mut usize,
        prev_dc: &mut i16,
        block_index: u32,
    ) -> Result<[i16; BLOCK_PIXELS], CodecError> {
        if *offset + 2 > bytes.len() {
            return Err(CodecError::TruncatedBlock(block_index));
        }
        let mut coeffs = [0i16; BLOCK_PIXELS];
        let dc_diff = i16::from_le_bytes(bytes[*offset..*offset + 2].try_into().unwrap());
        *offset += 2;
        let dc = prev_dc.wrapping_add(dc_diff);
        coeffs[0] = dc;
        *prev_dc = dc;

        let mut pos = 1usize;
        let mut finished = false;
        while pos < BLOCK_PIXELS {
            if *offset + 3 > bytes.len() {
                return Err(CodecError::TruncatedBlock(block_index));
            }
            let run = bytes[*offset];
            let value = i16::from_le_bytes(bytes[*offset + 1..*offset + 3].try_into().unwrap());
            *offset += 3;

            if run == RLE_EOB {
                finished = true;
                break;
            }

            let advance = run as usize;
            if pos + advance >= BLOCK_PIXELS {
                return Err(CodecError::RleOverflow(block_index));
            }
            pos += advance;
            coeffs[ZIG_ZAG[pos]] = value;
            pos += 1;
        }

        if !finished {
            if pos < BLOCK_PIXELS {
                return Err(CodecError::TruncatedBlock(block_index));
            }
            if *offset + 3 <= bytes.len() && bytes[*offset] == RLE_EOB {
                *offset += 3;
                return Ok(coeffs);
            }
            return Err(CodecError::MissingEndOfBlock(block_index));
        }
        Ok(coeffs)
    }

    #[inline]
    fn clamp_pixel(value: i32) -> u8 {
        value.clamp(0, 255) as u8
    }

    #[cfg(feature = "streaming-neural-filter")]
    fn apply_neural_filter(frame: &mut [u8], dims: FrameDimensions) {
        let width = usize::from(dims.width);
        let height = usize::from(dims.height);
        if width == 0 || height == 0 {
            return;
        }
        let len = width
            .checked_mul(height)
            .unwrap_or_default()
            .min(frame.len());
        if len == 0 || frame.len() < len {
            return;
        }

        const KERNEL: [i8; 9] = [1, 2, 1, 2, 4, 2, 1, 2, 1];
        const SHIFT: i32 = 4;
        const BIAS: i32 = 8;
        let mut out = vec![0u8; len];
        for y in 0..height {
            for x in 0..width {
                let mut acc = 0i32;
                for ky in 0..3 {
                    for kx in 0..3 {
                        let nx = x.saturating_add(kx).saturating_sub(1).min(width - 1);
                        let ny = y.saturating_add(ky).saturating_sub(1).min(height - 1);
                        let pixel = frame[ny * width + nx] as i32;
                        let weight = KERNEL[ky * 3 + kx] as i32;
                        acc += pixel * weight;
                    }
                }
                let value = ((acc + BIAS) >> SHIFT).clamp(0, 255) as u8;
                out[y * width + x] = value;
            }
        }
        frame[..len].copy_from_slice(&out);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn layout_channel_count(layout: AudioLayout) -> usize {
        match layout {
            AudioLayout::Mono => 1,
            AudioLayout::Stereo => 2,
            AudioLayout::FirstOrderAmbisonics => 4,
        }
    }

    #[derive(Clone, Copy, Debug)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    pub struct AudioEncoderConfig {
        pub sample_rate: u32,
        pub frame_samples: u16,
        pub layout: AudioLayout,
        pub fec_level: u8,
        pub target_bitrate: Option<u32>,
        pub backend: iroha_audio::BackendPreference,
    }

    impl Default for AudioEncoderConfig {
        fn default() -> Self {
            Self {
                sample_rate: 48_000,
                frame_samples: 240,
                layout: AudioLayout::Stereo,
                fec_level: 0,
                target_bitrate: None,
                backend: iroha_audio::BackendPreference::Auto,
            }
        }
    }

    impl AudioEncoderConfig {
        #[must_use]
        pub fn channel_count(&self) -> usize {
            self.layout.channel_count()
        }
    }

    pub struct AudioEncoder {
        config: AudioEncoderConfig,
        backend: iroha_audio::Encoder,
    }

    impl AudioEncoder {
        pub fn new(config: AudioEncoderConfig) -> Result<Self, AudioCodecError> {
            let backend = iroha_audio::Encoder::new(iroha_audio::EncoderConfig {
                sample_rate: config.sample_rate,
                frame_samples: config.frame_samples,
                layout: config.layout.into(),
                fec_level: config.fec_level,
                target_bitrate: config.target_bitrate,
                backend: config.backend,
            })
            .map_err(AudioCodecError::from_backend)?;
            Ok(Self { config, backend })
        }

        pub fn encode_frame(
            &mut self,
            sequence: u64,
            timestamp_ns: u64,
            pcm: &[i16],
        ) -> Result<AudioFrame, AudioCodecError> {
            let payload = self
                .backend
                .encode(pcm)
                .map_err(AudioCodecError::from_backend)?;
            Ok(AudioFrame {
                sequence,
                timestamp_ns,
                fec_level: self.config.fec_level,
                channel_layout: self.config.layout,
                payload,
            })
        }
    }

    pub struct AudioDecoder {
        config: AudioEncoderConfig,
        backend: iroha_audio::Decoder,
    }

    impl AudioDecoder {
        pub fn new(config: AudioEncoderConfig) -> Result<Self, AudioCodecError> {
            let backend = iroha_audio::Decoder::new(iroha_audio::EncoderConfig {
                sample_rate: config.sample_rate,
                frame_samples: config.frame_samples,
                layout: config.layout.into(),
                fec_level: config.fec_level,
                target_bitrate: None,
                backend: iroha_audio::BackendPreference::Auto,
            })
            .map_err(AudioCodecError::from_backend)?;
            Ok(Self { config, backend })
        }

        pub fn decode_frame(&mut self, frame: &AudioFrame) -> Result<Vec<i16>, AudioCodecError> {
            if frame.channel_layout != self.config.layout {
                return Err(AudioCodecError::LayoutMismatch(
                    AudioCodecLayoutMismatchInfo {
                        expected: self.config.layout,
                        found: frame.channel_layout,
                    },
                ));
            }
            self.backend
                .decode(&frame.payload)
                .map_err(AudioCodecError::from_backend)
        }
    }

    fn checked_chunk_count(chunk_count: usize) -> Result<u16, SegmentError> {
        u16::try_from(chunk_count).map_err(|_| {
            SegmentError::ChunkCountOverflow(ChunkCountOverflowInfo {
                found: saturating_usize_to_u32(chunk_count),
            })
        })
    }

    pub fn verify_segment(
        header: &SegmentHeader,
        descriptors: &[ChunkDescriptor],
        chunks: &[Vec<u8>],
        audio: Option<&SegmentAudio>,
    ) -> Result<(), SegmentError> {
        if descriptors.len() != chunks.len() {
            return Err(SegmentError::CountMismatch(ChunkListCountMismatch {
                descriptors: saturating_usize_to_u32(descriptors.len()),
                chunks: saturating_usize_to_u32(chunks.len()),
            }));
        }

        let descriptor_count = checked_chunk_count(descriptors.len())?;
        if header.chunk_count != descriptor_count {
            return Err(SegmentError::HeaderCountMismatch(
                HeaderDescriptorCountMismatch {
                    header: header.chunk_count,
                    actual: saturating_usize_to_u32(descriptors.len()),
                },
            ));
        }

        let mut payload_refs = Vec::with_capacity(descriptors.len());
        let mut expected_offset = 0u32;
        let mut last_chunk_id: Option<u16> = None;

        for (idx, (descriptor, chunk)) in descriptors.iter().zip(chunks.iter()).enumerate() {
            if let Some(last) = last_chunk_id
                && descriptor.chunk_id <= last
            {
                return Err(SegmentError::UnsortedChunkIds);
            }
            last_chunk_id = Some(descriptor.chunk_id);

            let chunk_len = u32::try_from(chunk.len())
                .map_err(|_| SegmentError::ChunkLengthOverflow(saturating_usize_to_u32(idx)))?;
            if descriptor.offset != expected_offset {
                return Err(SegmentError::DescriptorOffsetMismatch(
                    DescriptorOffsetDetails {
                        index: saturating_usize_to_u32(idx),
                        expected: expected_offset,
                        actual: descriptor.offset,
                    },
                ));
            }
            if descriptor.length != chunk_len {
                return Err(SegmentError::DescriptorLengthMismatch(
                    DescriptorLengthDetails {
                        index: saturating_usize_to_u32(idx),
                        descriptor: descriptor.length,
                        chunk: saturating_usize_to_u32(chunk.len()),
                    },
                ));
            }
            expected_offset = expected_offset
                .checked_add(chunk_len)
                .ok_or(SegmentError::OffsetOverflow(saturating_usize_to_u32(idx)))?;

            payload_refs.push((descriptor.chunk_id, chunk.as_slice()));
        }

        let commitments = chunk_commitments(header.segment_number, &payload_refs);

        for (idx, (descriptor, commitment)) in
            descriptors.iter().zip(commitments.iter()).enumerate()
        {
            if &descriptor.commitment != commitment {
                return Err(SegmentError::CommitmentMismatch(saturating_usize_to_u32(
                    idx,
                )));
            }
        }

        let root = merkle_root(&commitments)?;
        if header.chunk_merkle_root != root {
            return Err(SegmentError::MerkleMismatch);
        }

        match (header.audio_summary.as_ref(), audio) {
            (Some(summary), Some(track)) => {
                if track.summary.sample_rate != summary.sample_rate {
                    return Err(SegmentError::AudioSampleRateMismatch(
                        AudioSampleRateMismatchInfo {
                            expected: summary.sample_rate,
                            found: track.summary.sample_rate,
                        },
                    ));
                }
                if track.summary.frame_samples != summary.frame_samples {
                    return Err(SegmentError::AudioFrameSamplesMismatch(
                        AudioFrameSamplesMismatchInfo {
                            expected: summary.frame_samples,
                            found: track.summary.frame_samples,
                        },
                    ));
                }
                if track.summary.frame_duration_ns != summary.frame_duration_ns {
                    return Err(SegmentError::AudioFrameDurationMismatch(
                        AudioFrameDurationMismatchInfo {
                            expected: summary.frame_duration_ns,
                            found: track.summary.frame_duration_ns,
                        },
                    ));
                }
                if track.summary.fec_level != summary.fec_level {
                    return Err(SegmentError::AudioFecMismatch(AudioFecMismatchInfo {
                        expected: summary.fec_level,
                        found: track.summary.fec_level,
                    }));
                }
                if track.summary.layout != summary.layout {
                    return Err(SegmentError::AudioLayoutMismatch(AudioLayoutMismatchInfo {
                        expected: summary.layout,
                        found: track.summary.layout,
                    }));
                }
                if track.summary.frames_per_segment != summary.frames_per_segment {
                    return Err(SegmentError::AudioFrameCountMismatch(
                        AudioFrameCountMismatchInfo {
                            expected: summary.frames_per_segment,
                            found: track.summary.frames_per_segment,
                        },
                    ));
                }
                let expected_frames = usize::from(summary.frames_per_segment);
                if track.frames.len() != expected_frames {
                    return Err(SegmentError::AudioFrameCountMismatch(
                        AudioFrameCountMismatchInfo {
                            expected: summary.frames_per_segment,
                            found: track.frames.len().try_into().unwrap_or(u16::MAX),
                        },
                    ));
                }
                let frame_step = u64::from(summary.frame_duration_ns);
                for (idx, frame) in track.frames.iter().enumerate() {
                    if frame.channel_layout != summary.layout {
                        return Err(SegmentError::AudioLayoutMismatch(AudioLayoutMismatchInfo {
                            expected: summary.layout,
                            found: frame.channel_layout,
                        }));
                    }
                    if frame.fec_level != summary.fec_level {
                        return Err(SegmentError::AudioFecMismatch(AudioFecMismatchInfo {
                            expected: summary.fec_level,
                            found: frame.fec_level,
                        }));
                    }
                    let idx_u32 = saturating_usize_to_u32(idx);
                    let offset = frame_step
                        .checked_mul(idx as u64)
                        .ok_or(SegmentError::AudioTimestampOverflow(idx_u32))?;
                    let expected_ts = header
                        .timeline_start_ns
                        .checked_add(offset)
                        .ok_or(SegmentError::AudioTimestampOverflow(idx_u32))?;
                    let delta = frame.timestamp_ns.abs_diff(expected_ts);
                    if delta > AUDIO_SYNC_TOLERANCE_NS {
                        return Err(SegmentError::AudioTimestampMismatch(
                            AudioTimestampMismatchInfo {
                                index: idx_u32,
                                expected: expected_ts,
                                found: frame.timestamp_ns,
                            },
                        ));
                    }
                }
            }
            (Some(_), None) => return Err(SegmentError::AudioSummaryMissing),
            (None, Some(_)) => return Err(SegmentError::AudioSummaryUnexpected),
            (None, None) => {}
        }

        Ok(())
    }

    #[derive(Clone, Copy)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    struct OffsetMeta {
        offset: u32,
        length: u32,
    }

    fn compute_offsets(chunks: &[Vec<u8>]) -> Vec<OffsetMeta> {
        let mut offset = 0u32;
        chunks
            .iter()
            .map(|chunk| {
                let len = chunk.len() as u32;
                let meta = OffsetMeta {
                    offset,
                    length: len,
                };
                offset = offset.saturating_add(len);
                meta
            })
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use std::{str::FromStr, sync::Arc};

        use proptest::{collection::vec as pvec, prelude::*};

        use super::*;
        use crate::streaming::{Hash, chunk::BaselineDecoder};

        fn hash_seed(seed: u8) -> Hash {
            let mut bytes = [0u8; 32];
            bytes.fill(seed);
            bytes
        }

        const JPEG_SAMPLE_BLOCK: [i16; BLOCK_PIXELS] = [
            52, 55, 61, 66, 70, 61, 64, 73, //
            63, 59, 55, 90, 109, 85, 69, 72, //
            62, 59, 68, 113, 144, 104, 66, 73, //
            63, 58, 71, 122, 154, 106, 70, 69, //
            67, 61, 68, 104, 126, 88, 68, 70, //
            79, 65, 60, 70, 77, 68, 58, 75, //
            85, 71, 64, 59, 55, 61, 65, 83, //
            87, 79, 69, 68, 65, 76, 78, 94,
        ];

        const JPEG_SAMPLE_DCT_Q4: [i32; BLOCK_PIXELS] = [
            609, -30, -61, 27, 56, -20, -2, 0, 4, -22, -61, 10, 13, -7, -9, 5, -47, 7, 77, -25,
            -29, 10, 5, -6, -49, 12, 34, -15, -10, 6, 2, 2, 12, -7, -13, -4, -2, 2, -3, 3, -8, 3,
            2, -6, -2, 1, 4, 2, -1, 0, 0, -2, -1, -3, 4, -1, 0, 0, -1, -4, -1, 0, 1, 2,
        ];

        const JPEG_SAMPLE_QUANT_Q4: [i16; BLOCK_PIXELS] = [
            10, -1, -2, 0, 1, 0, 0, 0, //
            0, 0, -1, 0, 0, 0, 0, 0, //
            -1, 0, 1, 0, 0, 0, 0, 0, //
            -1, 0, 0, 0, 0, 0, 0, 0, //
            0, 0, 0, 0, 0, 0, 0, 0, //
            0, 0, 0, 0, 0, 0, 0, 0, //
            0, 0, 0, 0, 0, 0, 0, 0, //
            0, 0, 0, 0, 0, 0, 0, 0,
        ];

        const JPEG_SAMPLE_DEQUANT_Q4: [i32; BLOCK_PIXELS] = [
            640, -44, -80, 0, 96, 0, 0, 0, //
            0, 0, -56, 0, 0, 0, 0, 0, //
            -56, 0, 64, 0, 0, 0, 0, 0, //
            -56, 0, 0, 0, 0, 0, 0, 0, //
            0, 0, 0, 0, 0, 0, 0, 0, //
            0, 0, 0, 0, 0, 0, 0, 0, //
            0, 0, 0, 0, 0, 0, 0, 0, //
            0, 0, 0, 0, 0, 0, 0, 0,
        ];

        const JPEG_SAMPLE_RECON_Q4: [i32; BLOCK_PIXELS] = [
            55, 39, 51, 85, 88, 60, 52, 70, //
            64, 52, 69, 107, 110, 78, 65, 80, //
            72, 64, 88, 130, 133, 97, 77, 87, //
            70, 64, 90, 134, 137, 99, 77, 85, //
            64, 55, 77, 118, 121, 86, 68, 79, //
            67, 51, 63, 96, 99, 71, 64, 82, //
            82, 57, 57, 81, 84, 65, 70, 97, //
            97, 66, 57, 76, 79, 66, 79, 112,
        ];

        const GOLDEN_Q4_CHUNK: [u8; 30] = [
            0, 0, 0, 0, 232, 3, 0, 0, 0, 0, 0, 0, 0, 4, 1, 0, 14, 0, 0, 255, 255, 0, 247, 255, 6,
            255, 255, 255, 0, 0,
        ];

        #[test]
        fn encode_and_verify_roundtrip() {
            let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
            let dims = encoder.config.frame_dimensions;
            let frames = vec![
                RawFrame::new(dims, vec![1u8; dims.pixel_count()]).expect("frame"),
                RawFrame::new(dims, vec![2u8; dims.pixel_count()]).expect("frame"),
            ];
            let encoded = encoder
                .encode_segment(5, 1_000, 3, &frames, None)
                .expect("encode");

            assert_eq!(encoded.header.segment_number, 5);
            assert_eq!(encoded.header.chunk_count, 2);
            assert_eq!(encoded.descriptors.len(), 2);
            assert_eq!(encoded.descriptors[0].offset, 0);
            assert_eq!(
                encoded.descriptors[1].offset,
                encoded.chunks[0].len() as u32
            );

            verify_segment(
                &encoded.header,
                &encoded.descriptors,
                &encoded.chunks,
                encoded.audio.as_ref(),
            )
            .expect("verification");
        }

        #[test]
        fn encode_segment_with_chroma_roundtrips_payload() {
            let dims = FrameDimensions::new(16, 16);
            let frame = RawFrame::new(dims, vec![0xAA; dims.pixel_count()]).expect("frame");
            let chroma_len = dims.pixel_count() / 4;
            let chroma_frame =
                Chroma420Frame::new(dims, vec![0x10; chroma_len], vec![0xF0; chroma_len])
                    .expect("chroma");
            let config = BaselineEncoderConfig {
                frame_dimensions: dims,
                frames_per_segment: 1,
                frame_duration_ns: 10_000,
                quantizer: 12,
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config.clone());
            let segment = encoder
                .encode_segment_with_chroma(
                    5,
                    0,
                    3,
                    &[frame],
                    Some(std::slice::from_ref(&chroma_frame)),
                    None,
                )
                .expect("encode with chroma");
            let chunk = segment.chunks.first().expect("chunk");
            assert!(
                chunk.len() > FRAME_HEADER_LEN,
                "chunk should include chroma payload bytes"
            );

            let decoder = BaselineDecoder::new(dims, config.frame_duration_ns);
            let decoded = decoder.decode_segment(&segment).expect("decode");
            assert_eq!(decoded.len(), 1, "single frame should decode");
            let decoded_chroma = decoded[0]
                .chroma
                .as_ref()
                .expect("chroma should be present in decoded frame");
            assert_eq!(decoded_chroma.u.len(), chroma_len);
            assert_eq!(decoded_chroma.v.len(), chroma_len);
            assert!(
                decoded_chroma.u.iter().any(|&value| value != 128),
                "chroma decode should not fall back to neutral fill"
            );
            assert!(
                decoded_chroma.v.iter().any(|&value| value != 128),
                "chroma decode should not fall back to neutral fill"
            );
        }

        #[test]
        fn encode_segment_honors_explicit_duration() {
            let config = BaselineEncoderConfig {
                frame_dimensions: FrameDimensions::new(4, 4),
                duration_ns: 999_000,
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config.clone());

            let frame = RawFrame::new(config.frame_dimensions, vec![0xEE; 16]).unwrap();
            let encoded = encoder
                .encode_segment(6, 10_000, 2, &[frame], None)
                .expect("encode");
            assert_eq!(encoded.header.duration_ns, config.duration_ns);
        }

        #[test]
        fn verify_detects_tampered_chunk() {
            let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
            let dims = encoder.config.frame_dimensions;
            let frames = vec![
                RawFrame::new(dims, vec![10u8; dims.pixel_count()]).expect("frame"),
                RawFrame::new(dims, vec![11u8; dims.pixel_count()]).expect("frame"),
            ];
            let mut encoded = encoder
                .encode_segment(7, 2_000, 4, &frames, None)
                .expect("encode");
            encoded.chunks[0][0] ^= 0xFF;
            let err = verify_segment(
                &encoded.header,
                &encoded.descriptors,
                &encoded.chunks,
                encoded.audio.as_ref(),
            )
            .expect_err("should fail");
            assert!(matches!(err, SegmentError::CommitmentMismatch(0)));
        }

        #[test]
        fn verify_rejects_unsorted_chunk_ids() {
            let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
            let dims = encoder.config.frame_dimensions;
            let frames = vec![
                RawFrame::new(dims, vec![3u8; dims.pixel_count()]).expect("frame"),
                RawFrame::new(dims, vec![4u8; dims.pixel_count()]).expect("frame"),
            ];
            let mut encoded = encoder
                .encode_segment(9, 3_000, 7, &frames, None)
                .expect("encode");
            encoded.descriptors[0].chunk_id = encoded.descriptors[1].chunk_id;
            let err = verify_segment(
                &encoded.header,
                &encoded.descriptors,
                &encoded.chunks,
                encoded.audio.as_ref(),
            )
            .expect_err("unsorted ids must fail");
            assert!(matches!(err, SegmentError::UnsortedChunkIds));
        }

        #[test]
        fn verify_rejects_header_count_mismatch() {
            let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
            let dims = encoder.config.frame_dimensions;
            let frames = vec![RawFrame::new(dims, vec![0xAB; dims.pixel_count()]).expect("frame")];
            let mut encoded = encoder
                .encode_segment(11, 5_000, 8, &frames, None)
                .expect("encode");
            encoded.header.chunk_count = 0;
            let err = verify_segment(
                &encoded.header,
                &encoded.descriptors,
                &encoded.chunks,
                encoded.audio.as_ref(),
            )
            .expect_err("header mismatch must fail");
            assert!(matches!(
                err,
                SegmentError::HeaderCountMismatch(details)
                    if details.header == 0 && details.actual == 1
            ));
        }

        #[test]
        fn verify_rejects_descriptor_offset_mismatch() {
            let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
            let dims = encoder.config.frame_dimensions;
            let frames = vec![
                RawFrame::new(dims, vec![0x01; dims.pixel_count()]).expect("frame"),
                RawFrame::new(dims, vec![0x02; dims.pixel_count()]).expect("frame"),
            ];
            let mut encoded = encoder
                .encode_segment(13, 7_000, 9, &frames, None)
                .expect("encode");
            encoded.descriptors[1].offset += 1;
            let err = verify_segment(
                &encoded.header,
                &encoded.descriptors,
                &encoded.chunks,
                encoded.audio.as_ref(),
            )
            .expect_err("offset mismatch must fail");
            assert!(matches!(
                err,
                SegmentError::DescriptorOffsetMismatch(details) if details.index == 1
            ));
        }

        #[test]
        fn verify_rejects_descriptor_length_mismatch() {
            let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
            let dims = encoder.config.frame_dimensions;
            let frames = vec![
                RawFrame::new(dims, vec![0x05; dims.pixel_count()]).expect("frame"),
                RawFrame::new(dims, vec![0x06; dims.pixel_count()]).expect("frame"),
            ];
            let mut encoded = encoder
                .encode_segment(17, 8_000, 10, &frames, None)
                .expect("encode");
            encoded.descriptors[0].length -= 1;
            let err = verify_segment(
                &encoded.header,
                &encoded.descriptors,
                &encoded.chunks,
                encoded.audio.as_ref(),
            )
            .expect_err("length mismatch must fail");
            assert!(matches!(
                err,
                SegmentError::DescriptorLengthMismatch(details) if details.index == 0
            ));
        }

        #[test]
        fn decode_rejects_pts_mismatch() {
            let config = BaselineEncoderConfig {
                frame_dimensions: FrameDimensions::new(4, 4),
                frame_duration_ns: 40_000_000,
                frames_per_segment: 2,
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config.clone());

            let frames = vec![
                RawFrame::new(config.frame_dimensions, vec![0x11; 16]).unwrap(),
                RawFrame::new(config.frame_dimensions, vec![0x22; 16]).unwrap(),
            ];
            let mut segment = encoder
                .encode_segment(19, 1_000, 6, &frames, None)
                .expect("encode");
            segment.header.timeline_start_ns += 1;

            let decoder = BaselineDecoder::new(config.frame_dimensions, config.frame_duration_ns);
            let err = decoder
                .decode_segment(&segment)
                .expect_err("timeline mismatch should be detected");
            assert!(matches!(
                err,
                CodecError::FramePtsMismatch(details) if details.index == 0
            ));
        }

        #[test]
        fn encode_segment_detects_pts_overflow() {
            let config = BaselineEncoderConfig {
                frame_dimensions: FrameDimensions::new(2, 2),
                frame_duration_ns: u32::MAX,
                frames_per_segment: 2,
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config.clone());

            let frames: Vec<RawFrame> = (0..2)
                .map(|_| {
                    RawFrame::new(
                        config.frame_dimensions,
                        vec![0x7Fu8; config.frame_dimensions.pixel_count()],
                    )
                    .unwrap()
                })
                .collect();

            let timeline_start_ns = u64::MAX - u64::from(config.frame_duration_ns) + 1;
            let err = encoder
                .encode_segment(23, timeline_start_ns, 4, &frames, None)
                .expect_err("overflow must be rejected");
            assert!(matches!(err, CodecError::FramePtsOverflow(1)));
        }

        #[test]
        fn merkle_root_changes_on_chunk_mutation() {
            let dims = FrameDimensions::new(4, 4);
            let frames = vec![
                RawFrame::new(dims, vec![1u8; 16]).unwrap(),
                RawFrame::new(dims, vec![2u8; 16]).unwrap(),
            ];
            let mut encoder = BaselineEncoder::new(BaselineEncoderConfig {
                frame_dimensions: dims,
                ..BaselineEncoderConfig::default()
            });
            let encoded = encoder
                .encode_segment(1, 0, 1, &frames, None)
                .expect("encode");

            let payload_refs: Vec<(u16, &[u8])> = encoded
                .chunks
                .iter()
                .enumerate()
                .map(|(idx, chunk)| (idx as u16, chunk.as_slice()))
                .collect();
            let commitments_a = chunk_commitments(1, &payload_refs);
            let root_a = merkle_root(&commitments_a).expect("root");

            let mut chunks_b = encoded.chunks.clone();
            chunks_b[0][FRAME_HEADER_LEN] ^= 0x1;
            let payload_refs_b: Vec<(u16, &[u8])> = chunks_b
                .iter()
                .enumerate()
                .map(|(idx, chunk)| (idx as u16, chunk.as_slice()))
                .collect();
            let commitments_b = chunk_commitments(1, &payload_refs_b);
            let root_b = merkle_root(&commitments_b).expect("root");
            assert_ne!(root_a, root_b);
        }

        #[test]
        fn manifest_build_and_verify() {
            let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
            let dims = encoder.config.frame_dimensions;
            let frames = vec![
                RawFrame::new(dims, vec![1u8; dims.pixel_count()]).unwrap(),
                RawFrame::new(dims, vec![2u8; dims.pixel_count()]).unwrap(),
            ];
            let encoded = encoder
                .encode_segment(11, 5_000, 9, &frames, None)
                .expect("encode");

            let params = BaselineManifestParams {
                stream_id: hash_seed(3),
                protocol_version: 1,
                published_at: 1_700_000_000,
                da_endpoint: "/dns/publisher.example/quic".into(),
                privacy_routes: Vec::new(),
                public_metadata: StreamMetadata {
                    title: "Demo Stream".into(),
                    description: Some("Baseline manifest build test".into()),
                    access_policy_id: None,
                    tags: vec!["demo".into()],
                },
                capabilities: CapabilityFlags::from_bits(0b101),
                signature: [0u8; 64],
                fec_suite: FecScheme::Rs12_10,
                neural_bundle: None,
                transport_capabilities_hash: hash_seed(9),
            };
            let manifest = encoded.build_manifest(params);
            assert_eq!(manifest.segment_number, encoded.header.segment_number);
            assert_eq!(manifest.chunk_descriptors.len(), encoded.descriptors.len());
            encoded
                .verify_manifest(&manifest)
                .expect("manifest verification");
        }

        #[test]
        fn manifest_verify_detects_descriptor_mismatch() {
            let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
            let dims = encoder.config.frame_dimensions;
            let frame = RawFrame::new(dims, vec![0xAA; dims.pixel_count()]).unwrap();
            let encoded = encoder
                .encode_segment(21, 10_000, 5, &[frame], None)
                .expect("encode");
            let mut manifest = encoded.build_manifest(BaselineManifestParams {
                stream_id: hash_seed(1),
                da_endpoint: "/dns/test/quic".into(),
                ..BaselineManifestParams::default()
            });
            manifest.chunk_descriptors[0].length += 1;
            let err = encoded
                .verify_manifest(&manifest)
                .expect_err("should detect descriptor mismatch");
            assert!(matches!(err, ManifestError::DescriptorMismatch(0)));
        }

        #[test]
        fn manifest_verify_detects_chunk_root_mismatch() {
            let mut encoder = BaselineEncoder::new(BaselineEncoderConfig::default());
            let dims = encoder.config.frame_dimensions;
            let frames = vec![
                RawFrame::new(dims, vec![0x01; dims.pixel_count()]).unwrap(),
                RawFrame::new(dims, vec![0x02; dims.pixel_count()]).unwrap(),
            ];
            let encoded = encoder
                .encode_segment(4, 2_500, 3, &frames, None)
                .expect("encode");
            let mut manifest = encoded.build_manifest(BaselineManifestParams {
                stream_id: hash_seed(8),
                da_endpoint: "/dns/test/quic".into(),
                ..BaselineManifestParams::default()
            });
            manifest.chunk_root[0] ^= 0xFF;
            let err = encoded
                .verify_manifest(&manifest)
                .expect_err("should detect chunk root mismatch");
            assert!(matches!(err, ManifestError::ChunkRootMismatch));
        }

        #[test]
        fn manifest_verify_detects_audio_summary_mismatch() {
            let dims = FrameDimensions::new(4, 4);
            let audio_cfg = AudioEncoderConfig::default();
            let mut encoder = BaselineEncoder::new(BaselineEncoderConfig {
                frame_dimensions: dims,
                frames_per_segment: 1,
                frame_duration_ns: 5_000_000,
                audio: Some(audio_cfg),
                ..BaselineEncoderConfig::default()
            });
            let frame = RawFrame::new(dims, vec![0x55; dims.pixel_count()]).unwrap();
            let pcm = vec![0i16; audio_cfg.channel_count() * audio_cfg.frame_samples as usize];
            let segment = encoder
                .encode_segment(5, 10_000, 11, &[frame], Some(&pcm))
                .expect("encode segment");
            let mut manifest = segment.build_manifest(BaselineManifestParams {
                stream_id: hash_seed(7),
                da_endpoint: "/dns/audio/quic".into(),
                ..BaselineManifestParams::default()
            });
            manifest.audio_summary.as_mut().unwrap().sample_rate += 1;
            let err = segment
                .verify_manifest(&manifest)
                .expect_err("audio summary mismatch must fail");
            assert!(matches!(err, ManifestError::AudioSummaryMismatch));
        }

        #[test]
        fn encode_manifest_decode_roundtrip() {
            let config = BaselineEncoderConfig {
                frame_dimensions: FrameDimensions::new(4, 4),
                frame_duration_ns: 40_000_000,
                quantizer: 1,
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config.clone());

            let frames = vec![
                RawFrame::new(config.frame_dimensions, vec![0x10; 16]).unwrap(),
                RawFrame::new(config.frame_dimensions, vec![0x20; 16]).unwrap(),
            ];
            let segment = encoder
                .encode_segment(33, 500, 9, &frames, None)
                .expect("encode");

            let manifest = segment.build_manifest(BaselineManifestParams {
                stream_id: hash_seed(9),
                da_endpoint: "/dns/publisher/quic".into(),
                public_metadata: StreamMetadata {
                    title: "Roundtrip".into(),
                    description: None,
                    access_policy_id: None,
                    tags: vec!["rt".into()],
                },
                capabilities: CapabilityFlags::from_bits(0b1010),
                signature: [0u8; 64],
                ..BaselineManifestParams::default()
            });

            segment.verify_manifest(&manifest).expect("manifest verify");

            let decoder = BaselineDecoder::new(config.frame_dimensions, config.frame_duration_ns);
            let decoded = decoder.decode_segment(&segment).expect("decode");
            assert_eq!(decoded.len(), frames.len());
            assert_eq!(decoded[0].index, 0);
            assert_eq!(decoded[0].pts_ns, 500);
            assert_eq!(decoded[0].luma, frames[0].luma);
            assert_eq!(decoded[1].index, 1);
            assert_eq!(decoded[1].pts_ns, 500 + config.frame_duration_ns as u64);
            assert_eq!(decoded[1].luma, frames[1].luma);
        }

        #[test]
        fn audio_adpcm_roundtrip() {
            let mut encoder = AudioEncoder::new(AudioEncoderConfig {
                frame_samples: 64,
                ..AudioEncoderConfig::default()
            })
            .expect("encoder");
            let mut decoder = AudioDecoder::new(AudioEncoderConfig {
                frame_samples: 64,
                ..AudioEncoderConfig::default()
            })
            .expect("decoder");
            let channels = layout_channel_count(AudioLayout::Stereo);
            let mut pcm = Vec::with_capacity(channels * 64);
            for i in 0..64 {
                let angle = (i as f32) * std::f32::consts::PI * 2.0 / 32.0;
                let sample = (angle.sin() * 12_000.0) as i16;
                pcm.push(sample);
                pcm.push(sample.wrapping_mul(-1));
            }
            let frame = encoder.encode_frame(1, 1000, &pcm).expect("encode audio");
            let decoded = decoder.decode_frame(&frame).expect("decode audio");
            assert_eq!(decoded.len(), pcm.len());
            let max_err = decoded
                .iter()
                .zip(pcm.iter())
                .map(|(a, b)| (i32::from(*a) - i32::from(*b)).abs())
                .max()
                .unwrap();
            assert!(max_err <= 11000);
        }

        #[test]
        fn audio_encoder_errors_on_length() {
            let mut encoder = AudioEncoder::new(AudioEncoderConfig {
                frame_samples: 32,
                ..AudioEncoderConfig::default()
            })
            .expect("encoder");
            let pcm = vec![0i16; 10];
            let err = encoder
                .encode_frame(0, 0, &pcm)
                .expect_err("length mismatch must fail");
            assert!(matches!(err, AudioCodecError::InvalidPcmLength(_)));
        }

        #[test]
        fn baseline_encoder_rejects_zero_frame_samples() {
            let dims = FrameDimensions::new(4, 4);
            let audio_cfg = AudioEncoderConfig {
                frame_samples: 0,
                ..AudioEncoderConfig::default()
            };
            let config = BaselineEncoderConfig {
                frame_dimensions: dims,
                audio: Some(audio_cfg),
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config);
            let frame = RawFrame::new(dims, vec![0u8; dims.pixel_count()]).expect("frame");
            let err = encoder
                .encode_segment(1, 0, 0, &[frame], Some(&[]))
                .expect_err("zero frame_samples should error");
            assert!(matches!(
                err,
                CodecError::Audio(AudioCodecError::InvalidSampleCount(_))
            ));
        }

        #[test]
        fn baseline_encoder_produces_audio_summary() {
            let dims = FrameDimensions::new(4, 4);
            let audio_cfg = AudioEncoderConfig {
                frame_samples: 240,
                fec_level: 1,
                target_bitrate: Some(96_000),
                ..AudioEncoderConfig::default()
            };
            let frame_duration_ns = 5_000_000;
            let mut encoder = BaselineEncoder::new(BaselineEncoderConfig {
                frame_dimensions: dims,
                frames_per_segment: 2,
                frame_duration_ns,
                audio: Some(audio_cfg),
                ..BaselineEncoderConfig::default()
            });
            let frames = vec![
                RawFrame::new(dims, vec![0x10; dims.pixel_count()]).expect("frame"),
                RawFrame::new(dims, vec![0x20; dims.pixel_count()]).expect("frame"),
            ];
            let channels = audio_cfg.channel_count();
            let samples_per_frame = audio_cfg.frame_samples as usize * channels;
            let mut pcm = Vec::with_capacity(samples_per_frame * frames.len());
            pcm.extend(vec![100i16; samples_per_frame]);
            pcm.extend(vec![-100i16; samples_per_frame]);
            let segment = encoder
                .encode_segment(31, 2_000, 7, &frames, Some(&pcm))
                .expect("encode with audio");

            let audio = segment.audio.as_ref().expect("audio track present");
            assert_eq!(audio.summary.sample_rate, audio_cfg.sample_rate);
            assert_eq!(audio.summary.frame_samples, audio_cfg.frame_samples);
            assert_eq!(audio.summary.frame_duration_ns, frame_duration_ns);
            assert_eq!(audio.summary.frames_per_segment, 2);
            assert_eq!(audio.summary.layout, audio_cfg.layout);
            assert_eq!(audio.summary.fec_level, audio_cfg.fec_level);
            assert_eq!(segment.header.audio_summary, Some(audio.summary));
        }

        #[test]
        fn chroma_rejects_odd_dimensions() {
            let dims = FrameDimensions::new(7, 8);
            let err = Chroma420Frame::new(dims, Vec::new(), Vec::new())
                .expect_err("odd dimensions must be rejected for chroma");
            assert!(matches!(
                err,
                CodecError::ChromaDimensionsNotEven(info)
                    if info.width == 7 && info.height == 8
            ));
        }

        #[test]
        fn checked_frame_count_rejects_overflow() {
            let err = checked_frame_count(u16::MAX as usize + 1)
                .expect_err("frame count above u16 should fail");
            assert!(matches!(
                err,
                CodecError::FrameCountOverflow(info)
                    if info.found == u32::from(u16::MAX) + 1
            ));
        }

        #[test]
        fn checked_chunk_count_rejects_overflow() {
            let err = checked_chunk_count(u16::MAX as usize + 1)
                .expect_err("chunk count above u16 should fail");
            assert!(matches!(
                err,
                SegmentError::ChunkCountOverflow(info)
                    if info.found == u32::from(u16::MAX) + 1
            ));
        }

        #[test]
        fn audio_frame_cadence_mismatch_rejected() {
            let dims = FrameDimensions::new(8, 8);
            let audio_cfg = AudioEncoderConfig {
                sample_rate: 48_000,
                frame_samples: 240,
                layout: AudioLayout::Stereo,
                ..AudioEncoderConfig::default()
            };
            let config = BaselineEncoderConfig {
                frame_dimensions: dims,
                frame_duration_ns: 33_333_333,
                frames_per_segment: 1,
                audio: Some(audio_cfg),
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config);
            let frame = RawFrame::new(dims, vec![0x22; dims.pixel_count()]).expect("frame");
            let channels = audio_cfg.channel_count();
            let pcm = vec![0i16; audio_cfg.frame_samples as usize * channels];
            let err = encoder
                .encode_segment(1, 1_000, 1, &[frame], Some(&pcm))
                .expect_err("cadence mismatch must fail");
            assert!(matches!(
                err,
                CodecError::AudioFrameCadenceMismatch(info)
                    if info.expected == 1600 && info.found == 240
            ));
        }

        #[test]
        fn audio_frame_cadence_rounding_accepts_30fps() {
            let dims = FrameDimensions::new(8, 8);
            let audio_cfg = AudioEncoderConfig {
                sample_rate: 48_000,
                frame_samples: 1600,
                layout: AudioLayout::Stereo,
                ..AudioEncoderConfig::default()
            };
            let config = BaselineEncoderConfig {
                frame_dimensions: dims,
                frame_duration_ns: 33_333_333,
                frames_per_segment: 1,
                audio: Some(audio_cfg),
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config);
            let frame = RawFrame::new(dims, vec![0x33; dims.pixel_count()]).expect("frame");
            let channels = audio_cfg.channel_count();
            let pcm = vec![0i16; audio_cfg.frame_samples as usize * channels];
            encoder
                .encode_segment(2, 2_000, 2, &[frame], Some(&pcm))
                .expect("rounded cadence should pass");
        }

        #[test]
        fn bundled_entropy_stats_recorded() {
            let config = BaselineEncoderConfig {
                frame_dimensions: FrameDimensions::new(8, 8),
                frames_per_segment: 1,
                entropy_mode: EntropyMode::RansBundled,
                bundle_width: 2,
                ..BaselineEncoderConfig::default()
            };

            let mut encoder = BaselineEncoder::new(config.clone());
            let frame = RawFrame::new(
                config.frame_dimensions,
                vec![0x55; config.frame_dimensions.pixel_count()],
            )
            .expect("frame");

            let segment = encoder
                .encode_segment(1, 0, 7, &[frame], None)
                .expect("encode");
            assert_eq!(segment.header.entropy_mode, EntropyMode::RansBundled);

            let telemetry = encoder
                .bundled_telemetry()
                .expect("bundled telemetry recorded");
            assert_eq!(telemetry.stats.bundle_width, config.bundle_width);
            assert!(telemetry.stats.blocks_encoded > 0);
            assert!(telemetry.stats.dc_events >= telemetry.stats.blocks_encoded);
            assert!(telemetry.stats.flush_type_complete > 0);
            assert_eq!(
                telemetry.stats.flush_end_of_block,
                telemetry.stats.blocks_encoded
            );
            assert!(!telemetry.bundles.is_empty());
            assert!(!telemetry.ans_stream.is_empty());
            let tables = default_bundle_tables();
            assert_eq!(telemetry.tables_checksum, tables.checksum());
            let decoded =
                decode_bundle_stream(&telemetry.ans_stream, &telemetry.bundles, tables.as_ref())
                    .expect("bundled stream decodes");
            for (record, &decoded_bits) in telemetry.bundles.iter().zip(decoded.iter()) {
                let mask = (1u8 << record.bit_len) - 1;
                assert_eq!(record.bits & mask, decoded_bits & mask);
            }
            assert!(!telemetry.tokens.is_empty());
        }

        #[test]
        fn bundled_context_stats_cover_flushes() {
            let config = BaselineEncoderConfig {
                frame_dimensions: FrameDimensions::new(8, 8),
                frames_per_segment: 1,
                entropy_mode: EntropyMode::RansBundled,
                bundle_width: 3,
                ..BaselineEncoderConfig::default()
            };

            let mut encoder = BaselineEncoder::new(config.clone());
            let frame = RawFrame::new(
                config.frame_dimensions,
                vec![0x11; config.frame_dimensions.pixel_count()],
            )
            .expect("frame");

            encoder
                .encode_segment(2, 10, 9, &[frame], None)
                .expect("bundled encode");

            let telemetry = encoder
                .bundled_telemetry()
                .expect("bundled telemetry recorded");
            assert!(
                !telemetry.context_stats.is_empty(),
                "bundled telemetry should expose per-context stats"
            );
            assert!(
                telemetry
                    .context_stats
                    .iter()
                    .any(|ctx| ctx.flush_end_of_block > 0),
                "context stats should record at least one end-of-block flush"
            );
            assert!(
                telemetry
                    .context_stats
                    .iter()
                    .any(|ctx| ctx.flush_context_switch > 0),
                "context switches should be tracked when contexts change"
            );
            for stats in &telemetry.context_stats {
                let type_sum = stats.type_significance_only
                    + stats.type_sign_and_magnitude
                    + stats.type_sign_parity
                    + stats.type_sign_parity_level
                    + stats.type_significance_rle;
                assert_eq!(
                    stats.bundles_total, type_sum,
                    "per-context bundle totals must match recorded type counters"
                );
                assert_eq!(
                    stats.flush_type_complete, stats.bundles_total,
                    "type-complete flushes should track per-context bundle totals"
                );
            }
        }

        #[test]
        fn bundle_stream_recorder_counts_zero_runs_and_tokens() {
            let dims = FrameDimensions::new(8, 8);
            let tables = default_bundle_tables();
            let mut recorder = BundleStreamRecorder::new(
                2,
                dims,
                4,
                Arc::clone(&tables),
                BundleAcceleration::None,
                0,
                None,
            );
            let mut coeffs = [0i16; BLOCK_PIXELS];
            coeffs[ZIG_ZAG[1]] = 5;
            coeffs[ZIG_ZAG[4]] = -2;

            recorder.record_block(0, &coeffs, FrameType::Predicted);
            recorder.record_dc(3);
            recorder.record_ac(0, 5);
            recorder.record_ac(2, -2);
            recorder.record_eob();

            let telemetry = recorder.finish();
            assert_eq!(telemetry.stats.blocks_encoded, 1);
            assert_eq!(telemetry.stats.ac_events, 2);
            assert_eq!(telemetry.stats.zero_run_total, 2);
            assert_eq!(telemetry.stats.max_zero_run, 2);
            assert_eq!(telemetry.stats.nonzero_levels, 2);
            assert_eq!(telemetry.tokens.len(), 4);
            assert!(matches!(telemetry.tokens[0], BundledToken::DcDiff(3)));
            assert!(matches!(
                telemetry.tokens[1],
                BundledToken::Ac { run: 0, value } if value == 5
            ));
            assert!(matches!(
                telemetry.tokens[2],
                BundledToken::Ac { run: 2, value } if value == -2
            ));
            assert!(matches!(telemetry.tokens[3], BundledToken::EndOfBlock));
            assert!(
                telemetry.stats.significance_rle > 0,
                "zero runs should emit RLE bundles"
            );
        }

        #[test]
        fn bundle_stream_recorder_flushes_zero_only_blocks() {
            let dims = FrameDimensions::new(8, 8);
            let tables = default_bundle_tables();
            let mut recorder = BundleStreamRecorder::new(
                2,
                dims,
                4,
                Arc::clone(&tables),
                BundleAcceleration::None,
                0,
                None,
            );
            let coeffs = [0i16; BLOCK_PIXELS];

            recorder.record_block(0, &coeffs, FrameType::Intra);
            recorder.record_dc(0);
            recorder.record_eob();

            let telemetry = recorder.finish();
            assert_eq!(telemetry.stats.blocks_encoded, 1);
            let expected_slots =
                u16::try_from(BLOCK_PIXELS - 1).expect("block pixel count fits u16");
            let consumed: u16 = telemetry
                .bundles
                .iter()
                .filter(|record| matches!(record.bundle_type, BundleType::SignificanceRle))
                .map(|record| u16::from(record.bits))
                .sum();
            assert_eq!(
                consumed, expected_slots,
                "run-length bundles must cover every AC slot"
            );
            assert_eq!(
                telemetry.stats.flush_type_complete,
                telemetry.bundles.len() as u64
            );
            assert_eq!(telemetry.stats.flush_end_of_block, 1);
            assert_eq!(
                telemetry.stats.significance_rle,
                telemetry.bundles.len() as u64
            );
        }

        #[test]
        fn bundle_stream_recorder_accumulates_significance_stats() {
            let dims = FrameDimensions::new(8, 8);
            let tables = default_bundle_tables();
            // Use width four so both parity and geq2 counters are active.
            let mut recorder = BundleStreamRecorder::new(
                4,
                dims,
                4,
                Arc::clone(&tables),
                BundleAcceleration::None,
                0,
                None,
            );
            let mut coeffs = [0i16; BLOCK_PIXELS];
            // Populate the first three AC slots with deterministic values.
            coeffs[ZIG_ZAG[1]] = 3; // odd positive
            coeffs[ZIG_ZAG[2]] = -4; // even negative (>= 2)
            coeffs[ZIG_ZAG[3]] = 1; // odd positive

            recorder.record_block(0, &coeffs, FrameType::Intra);
            recorder.record_dc(7);
            recorder.record_ac(0, 3);
            recorder.record_ac(0, -4);
            recorder.record_ac(0, 1);
            recorder.record_eob();

            let telemetry = recorder.finish();
            let total_ac = u64::try_from(BLOCK_PIXELS - 1).expect("block size fits u64");
            assert_eq!(telemetry.stats.significance_one, 3);
            assert_eq!(telemetry.stats.significance_zero, total_ac - 3);
            assert_eq!(telemetry.stats.sign_positive, total_ac - 1);
            assert_eq!(telemetry.stats.sign_negative, 1);
            assert_eq!(telemetry.stats.parity_one, 2);
            assert_eq!(telemetry.stats.geq2_one, 2);
        }

        #[test]
        fn bundle_prefetch_keeps_stream_deterministic() {
            let dims = FrameDimensions::new(8, 8);
            let tables = default_bundle_tables();
            let mut recorder = BundleStreamRecorder::new(
                2,
                dims,
                6,
                Arc::clone(&tables),
                BundleAcceleration::None,
                3,
                None,
            );

            for block_idx in 0..6 {
                let mut coeffs = [0i16; BLOCK_PIXELS];
                coeffs[ZIG_ZAG[1]] = (block_idx as i16) + 1;
                recorder.record_block(block_idx, &coeffs, FrameType::Predicted);
                recorder.record_dc(block_idx as i16);
                recorder.record_ac(0, coeffs[ZIG_ZAG[1]]);
                recorder.record_eob();
            }

            let telemetry = recorder.finish();
            let baseline_stream =
                encode_bundle_stream_scalar(tables.as_ref(), &telemetry.bundles, 0);
            let prefetch_stream = encode_bundle_stream_scalar(
                tables.as_ref(),
                &telemetry.bundles,
                telemetry.prefetch_distance,
            );
            let decoded_prefetch =
                decode_bundle_stream(&telemetry.ans_stream, &telemetry.bundles, tables.as_ref())
                    .expect("prefetch stream decodes");
            let decoded_prefetch_stream =
                decode_bundle_stream(&prefetch_stream, &telemetry.bundles, tables.as_ref())
                    .expect("prefetch stream decodes");
            let decoded_baseline =
                decode_bundle_stream(&baseline_stream, &telemetry.bundles, tables.as_ref())
                    .expect("baseline stream decodes");
            assert_eq!(decoded_prefetch, decoded_baseline);
            assert_eq!(decoded_prefetch_stream, decoded_baseline);
            assert_eq!(telemetry.prefetch_distance, 3);
            assert_eq!(telemetry.acceleration, BundleAcceleration::None);
        }

        fn sample_bundles() -> Vec<BundleRecord> {
            vec![
                BundleRecord {
                    bundle_type: BundleType::SignificanceOnly,
                    context: BundleContextId::new(1),
                    bits: 0b1,
                    bit_len: 1,
                    flush: BundleFlushReason::TypeComplete,
                },
                BundleRecord {
                    bundle_type: BundleType::SignAndMagnitude,
                    context: BundleContextId::new(2),
                    bits: 0b10,
                    bit_len: 2,
                    flush: BundleFlushReason::TypeComplete,
                },
                BundleRecord {
                    bundle_type: BundleType::SignParity,
                    context: BundleContextId::new(3),
                    bits: 0b101,
                    bit_len: 3,
                    flush: BundleFlushReason::TypeComplete,
                },
            ]
        }

        #[test]
        fn bundle_stream_simd_roundtrips() {
            let tables = default_bundle_tables();
            let bundles = sample_bundles();
            let stream = encode_bundle_stream_simd(tables.as_ref(), &bundles, 0);
            assert!(stream.starts_with(&SIMD_BUNDLE_MAGIC));
            let decoded = decode_bundle_stream(&stream, &bundles, tables.as_ref()).expect("decode");
            let expected: Vec<u8> = bundles
                .iter()
                .map(|record| record.bits & ((1u8 << record.bit_len) - 1))
                .collect();
            assert_eq!(decoded, expected);
        }

        #[test]
        fn bundle_stream_acceleration_matches_header() {
            let tables = default_bundle_tables();
            let bundles = sample_bundles();
            let (stream, accel) = encode_bundle_stream_with_opts(
                tables.as_ref(),
                &bundles,
                BundleAcceleration::CpuSimd,
                0,
            );
            let expected = if stream.starts_with(&SIMD_BUNDLE_MAGIC) {
                BundleAcceleration::CpuSimd
            } else {
                BundleAcceleration::None
            };
            assert_eq!(accel, expected);
        }

        #[test]
        fn rle_bundles_roundtrip_via_ans() {
            let tables = default_bundle_tables();
            let bundles = vec![
                BundleRecord {
                    bundle_type: BundleType::SignificanceRle,
                    context: BundleContextId::new(1),
                    bits: 1,
                    bit_len: 3,
                    flush: BundleFlushReason::TypeComplete,
                },
                BundleRecord {
                    bundle_type: BundleType::SignificanceRle,
                    context: BundleContextId::new(2),
                    bits: 3,
                    bit_len: 3,
                    flush: BundleFlushReason::TypeComplete,
                },
            ];
            let stream = encode_bundle_stream_scalar(tables.as_ref(), &bundles, 0);
            let decoded =
                decode_bundle_stream(&stream, &bundles, tables.as_ref()).expect("decode rle");
            for (record, decoded_bits) in bundles.iter().zip(decoded.iter()) {
                let mask = (1u8 << record.bit_len) - 1;
                assert_eq!(record.bits & mask, decoded_bits & mask);
            }
        }

        #[test]
        fn rle_all_one_symbols_roundtrip() {
            let tables = default_bundle_tables();
            let mut bundles = Vec::with_capacity(BLOCK_PIXELS - 1);
            for idx in 0..(BLOCK_PIXELS - 1) {
                bundles.push(BundleRecord {
                    bundle_type: BundleType::SignificanceRle,
                    context: BundleContextId::new(idx as u16),
                    bits: 1,
                    bit_len: 3,
                    flush: BundleFlushReason::TypeComplete,
                });
            }
            let stream = encode_bundle_stream_scalar(tables.as_ref(), &bundles, 0);
            let decoded =
                decode_bundle_stream(&stream, &bundles, tables.as_ref()).expect("rle decode");
            assert_eq!(decoded.len(), bundles.len());
            assert!(decoded.iter().all(|&value| value == 1));
        }

        #[test]
        fn simd_bundle_stream_matches_scalar_output() {
            if !cpu_simd_supported() {
                return;
            }
            let dims = FrameDimensions::new(8, 8);
            let tables = default_bundle_tables();
            let mut recorder = BundleStreamRecorder::new(
                3,
                dims,
                5,
                Arc::clone(&tables),
                BundleAcceleration::CpuSimd,
                2,
                None,
            );

            let mut coeffs = [0i16; BLOCK_PIXELS];
            coeffs[ZIG_ZAG[1]] = 5;
            coeffs[ZIG_ZAG[2]] = -3;
            coeffs[ZIG_ZAG[4]] = 4;
            recorder.record_block(0, &coeffs, FrameType::Intra);
            recorder.record_dc(1);
            recorder.record_ac(0, coeffs[ZIG_ZAG[1]]);
            recorder.record_ac(0, coeffs[ZIG_ZAG[2]]);
            recorder.record_ac(1, coeffs[ZIG_ZAG[4]]);
            recorder.record_eob();

            let telemetry = recorder.finish();
            let scalar_stream = encode_bundle_stream_scalar(tables.as_ref(), &telemetry.bundles, 0);
            let simd_decoded =
                decode_bundle_stream(&telemetry.ans_stream, &telemetry.bundles, tables.as_ref())
                    .expect("simd decode");
            let scalar_decoded =
                decode_bundle_stream(&scalar_stream, &telemetry.bundles, tables.as_ref())
                    .expect("scalar decode");
            assert_eq!(telemetry.acceleration, BundleAcceleration::None);
            assert_eq!(simd_decoded, scalar_decoded);
            assert_eq!(telemetry.prefetch_distance, 2);
        }

        #[test]
        fn bundled_manifest_requires_checksum() {
            let config = BaselineEncoderConfig {
                frame_dimensions: FrameDimensions::new(8, 8),
                frames_per_segment: 1,
                entropy_mode: EntropyMode::RansBundled,
                bundle_width: 2,
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config.clone());
            let frame = RawFrame::new(
                config.frame_dimensions,
                vec![0x33; config.frame_dimensions.pixel_count()],
            )
            .expect("frame");
            let segment = encoder
                .encode_segment(2, 0, 5, &[frame], None)
                .expect("bundled segment");
            let mut manifest = segment.build_manifest(BaselineManifestParams {
                stream_id: [0x20; 32],
                ..BaselineManifestParams::default()
            });
            assert!(
                manifest.entropy_tables_checksum.is_some(),
                "bundled manifest must carry checksum"
            );
            segment
                .verify_manifest(&manifest)
                .expect("checksum matches");
            manifest.entropy_tables_checksum = None;
            let err = segment
                .verify_manifest(&manifest)
                .expect_err("missing checksum must fail");
            assert!(matches!(err, ManifestError::EntropyTablesMismatch));
        }

        #[test]
        fn manifest_capabilities_follow_entropy_mode() {
            let bundled_config = BaselineEncoderConfig {
                frame_dimensions: FrameDimensions::new(8, 8),
                frames_per_segment: 1,
                entropy_mode: EntropyMode::RansBundled,
                bundle_width: 2,
                ..BaselineEncoderConfig::default()
            };
            let mut bundled_encoder = BaselineEncoder::new(bundled_config.clone());
            let bundled_frame = RawFrame::new(
                bundled_config.frame_dimensions,
                vec![0x21; bundled_config.frame_dimensions.pixel_count()],
            )
            .expect("frame");
            let bundled_segment = bundled_encoder
                .encode_segment(3, 0, 5, &[bundled_frame], None)
                .expect("bundled segment");
            let manifest = bundled_segment.build_manifest(BaselineManifestParams {
                capabilities: CapabilityFlags::from_bits(0),
                ..BaselineManifestParams::default()
            });
            assert!(
                manifest
                    .capabilities
                    .contains(CapabilityFlags::FEATURE_ENTROPY_BUNDLED),
                "bundled manifests must set FEATURE_ENTROPY_BUNDLED"
            );
        }

        #[test]
        fn bundled_manifest_missing_entropy_feature_bit_rejected() {
            let config = BaselineEncoderConfig {
                frame_dimensions: FrameDimensions::new(8, 8),
                frames_per_segment: 1,
                entropy_mode: EntropyMode::RansBundled,
                bundle_width: 2,
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config.clone());
            let frame = RawFrame::new(
                config.frame_dimensions,
                vec![0x33; config.frame_dimensions.pixel_count()],
            )
            .expect("frame");
            let segment = encoder
                .encode_segment(6, 0, 5, &[frame], None)
                .expect("bundled segment");
            let mut manifest = segment.build_manifest(BaselineManifestParams::default());
            manifest.capabilities = manifest
                .capabilities
                .remove(CapabilityFlags::FEATURE_ENTROPY_BUNDLED);
            let err = segment
                .verify_manifest(&manifest)
                .expect_err("missing bundled bit must fail manifest verification");
            assert!(matches!(
                err,
                ManifestError::CapabilityEntropyFlagMismatch {
                    required_bundled: true,
                    found_bundled: false
                }
            ));
        }

        #[test]
        fn bundled_manifest_missing_required_acceleration_bit_rejected() {
            let config = BaselineEncoderConfig {
                frame_dimensions: FrameDimensions::new(8, 8),
                frames_per_segment: 1,
                entropy_mode: EntropyMode::RansBundled,
                bundle_width: 2,
                bundle_acceleration: BundleAcceleration::CpuSimd,
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config);
            let frame = RawFrame::new(
                FrameDimensions::new(8, 8),
                vec![0x66; FrameDimensions::new(8, 8).pixel_count()],
            )
            .expect("frame");
            let segment = encoder
                .encode_segment(9, 0, 5, &[frame], None)
                .expect("bundled segment");
            let mut manifest = segment.build_manifest(BaselineManifestParams::default());
            manifest.capabilities = manifest
                .capabilities
                .remove(CapabilityFlags::FEATURE_BUNDLE_ACCEL_CPU_SIMD);
            let err = segment
                .verify_manifest(&manifest)
                .expect_err("missing acceleration bit must fail verification");
            assert!(matches!(
                err,
                ManifestError::CapabilityAccelerationFlagMismatch {
                    required_mask,
                    found_mask,
                    ..
                }
                if required_mask == CapabilityFlags::FEATURE_BUNDLE_ACCEL_CPU_SIMD
                    && found_mask == 0
            ));
        }

        #[test]
        fn bundled_manifest_with_wrong_acceleration_bit_rejected() {
            let config = BaselineEncoderConfig {
                frame_dimensions: FrameDimensions::new(8, 8),
                frames_per_segment: 1,
                entropy_mode: EntropyMode::RansBundled,
                bundle_width: 2,
                bundle_acceleration: BundleAcceleration::Gpu,
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config);
            let frame = RawFrame::new(
                FrameDimensions::new(8, 8),
                vec![0x77; FrameDimensions::new(8, 8).pixel_count()],
            )
            .expect("frame");
            let segment = encoder
                .encode_segment(10, 0, 5, &[frame], None)
                .expect("bundled segment");
            let mut manifest = segment.build_manifest(BaselineManifestParams::default());
            manifest.capabilities = manifest
                .capabilities
                .remove(CapabilityFlags::FEATURE_BUNDLE_ACCEL_GPU)
                .insert(CapabilityFlags::FEATURE_BUNDLE_ACCEL_CPU_SIMD);
            let err = segment
                .verify_manifest(&manifest)
                .expect_err("wrong acceleration bit must fail verification");
            assert!(matches!(
                err,
                ManifestError::CapabilityAccelerationFlagMismatch {
                    required_mask,
                    found_mask,
                    ..
                }
                if required_mask == CapabilityFlags::FEATURE_BUNDLE_ACCEL_GPU
                    && found_mask == CapabilityFlags::FEATURE_BUNDLE_ACCEL_CPU_SIMD
            ));
        }

        #[test]
        fn bundled_telemetry_roundtrips_tokens_via_ans_stream() {
            let config = BaselineEncoderConfig {
                frame_dimensions: FrameDimensions::new(8, 8),
                frames_per_segment: 1,
                entropy_mode: EntropyMode::RansBundled,
                bundle_width: 3,
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config);
            let frame = RawFrame::new(
                FrameDimensions::new(8, 8),
                vec![0x22; FrameDimensions::new(8, 8).pixel_count()],
            )
            .expect("frame");
            encoder
                .encode_segment(10, 0, 7, &[frame], None)
                .expect("bundled segment");
            let telemetry = encoder
                .bundled_telemetry()
                .expect("bundled telemetry recorded")
                .clone();
            let tables = default_bundle_tables();
            telemetry
                .verify_stream(tables.as_ref())
                .expect("bundled ANS stream verifies");
            let decoded = telemetry
                .decode_symbols(tables.as_ref())
                .expect("decoded symbols");
            assert_eq!(decoded.len(), telemetry.bundles.len());
        }

        #[test]
        fn bundled_telemetry_checksum_mismatch_signalled() {
            let config = BaselineEncoderConfig {
                frame_dimensions: FrameDimensions::new(8, 8),
                frames_per_segment: 1,
                entropy_mode: EntropyMode::RansBundled,
                bundle_width: 2,
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config);
            let frame = RawFrame::new(
                FrameDimensions::new(8, 8),
                vec![0x33; FrameDimensions::new(8, 8).pixel_count()],
            )
            .expect("frame");
            encoder
                .encode_segment(11, 0, 7, &[frame], None)
                .expect("bundled segment");
            let mut telemetry = encoder
                .bundled_telemetry()
                .expect("bundled telemetry recorded")
                .clone();
            telemetry.tables_checksum = [0xAA; 32];
            let tables = default_bundle_tables();
            let err = telemetry
                .verify_stream(tables.as_ref())
                .expect_err("checksum mismatch must be reported");
            assert!(matches!(err, BundleDecodeError::ChecksumMismatch { .. }));
        }

        #[test]
        fn segment_bundle_roundtrip_validates_payloads() {
            let dims = FrameDimensions::new(8, 8);
            let config = BaselineEncoderConfig {
                frame_dimensions: dims,
                frames_per_segment: 2,
                frame_duration_ns: 20_000_000,
                quantizer: 4,
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config.clone());
            let frame0 = RawFrame::new(dims, vec![0x55; dims.pixel_count()]).expect("frame zero");
            let frame1 = RawFrame::new(dims, vec![0x77; dims.pixel_count()]).expect("frame one");
            let uv_len = usize::from(dims.width / 2 * dims.height / 2);
            let chroma0 =
                Chroma420Frame::new(dims, vec![0x11; uv_len], vec![0x22; uv_len]).expect("chroma0");
            let chroma1 =
                Chroma420Frame::new(dims, vec![0x33; uv_len], vec![0x44; uv_len]).expect("chroma1");
            let segment = encoder
                .encode_segment(12, 1_000_000, 5, &[frame0, frame1], None)
                .expect("encode");
            let bundle = segment.to_bundle_with_chroma(
                config.frame_dimensions,
                config.frame_duration_ns,
                vec![chroma0.clone(), chroma1.clone()],
            );
            let (roundtrip, bundle_dims, bundle_duration, chroma_roundtrip) =
                bundle.into_segment_with_chroma().expect("bundle decodes");
            assert_eq!(bundle_dims, config.frame_dimensions);
            assert_eq!(bundle_duration, config.frame_duration_ns);
            assert_eq!(roundtrip.header, segment.header);
            assert_eq!(roundtrip.descriptors, segment.descriptors);
            assert_eq!(roundtrip.chunks, segment.chunks);
            assert_eq!(roundtrip.audio, segment.audio);
            assert_eq!(chroma_roundtrip, vec![chroma0, chroma1]);
        }

        #[test]
        fn rdo_perceptual_uses_softer_lambda() {
            let default = rdo_lambda_for_mode(RdoMode::DynamicProgramming, 20);
            let perceptual = rdo_lambda_for_mode(RdoMode::Perceptual, 20);
            assert!(
                perceptual < default,
                "perceptual lambda should be softer than default for SSIM bias"
            );
        }

        #[test]
        fn entropy_mode_parsing_rejects_unknown_strings() {
            assert_eq!(EntropyMode::from_str("rans"), Err(()));
            assert_eq!(EntropyMode::from_str("rans-unknown"), Err(()));
            assert_eq!(EntropyMode::from_str("cabac"), Err(()));
            assert_eq!(
                EntropyMode::from_str("rans_bundled"),
                Ok(EntropyMode::RansBundled)
            );
        }

        #[test]
        fn bundled_ans_roundtrip_order() {
            let records = vec![
                BundleRecord {
                    bundle_type: BundleType::SignificanceOnly,
                    context: BundleContextId::new(1),
                    bits: 0,
                    bit_len: 1,
                    flush: BundleFlushReason::TypeComplete,
                },
                BundleRecord {
                    bundle_type: BundleType::SignAndMagnitude,
                    context: BundleContextId::new(2),
                    bits: 0b11,
                    bit_len: 2,
                    flush: BundleFlushReason::TypeComplete,
                },
                BundleRecord {
                    bundle_type: BundleType::SignParity,
                    context: BundleContextId::new(3),
                    bits: 0b101,
                    bit_len: 3,
                    flush: BundleFlushReason::TypeComplete,
                },
                BundleRecord {
                    bundle_type: BundleType::SignParityLevel,
                    context: BundleContextId::new(4),
                    bits: 0b1110,
                    bit_len: 4,
                    flush: BundleFlushReason::TypeComplete,
                },
            ];
            let tables = default_bundle_tables();
            let stream = encode_bundle_stream(tables.as_ref(), &records);
            let decoded = decode_bundle_stream(&stream, &records, tables.as_ref())
                .expect("bundle stream decodes");
            assert_eq!(
                decoded,
                records
                    .iter()
                    .map(|r| r.bits & ((1u8 << r.bit_len) - 1))
                    .collect::<Vec<_>>()
            );
        }

        #[test]
        fn verify_segment_accepts_audio_within_tolerance() {
            let dims = FrameDimensions::new(4, 4);
            let audio_cfg = AudioEncoderConfig::default();
            let config = BaselineEncoderConfig {
                frame_dimensions: dims,
                frames_per_segment: 2,
                frame_duration_ns: 5_000_000,
                audio: Some(audio_cfg),
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config);
            let frames = vec![
                RawFrame::new(dims, vec![0x11; dims.pixel_count()]).expect("frame"),
                RawFrame::new(dims, vec![0x22; dims.pixel_count()]).expect("frame"),
            ];
            let channels = audio_cfg.channel_count();
            let samples_per_frame = audio_cfg.frame_samples as usize * channels;
            let pcm = vec![0i16; samples_per_frame * frames.len()];
            let segment = encoder
                .encode_segment(41, 3_000, 5, &frames, Some(&pcm))
                .expect("encode segment");

            let mut skewed = segment.clone();
            if let Some(audio) = skewed.audio.as_mut() {
                audio.frames[0].timestamp_ns += AUDIO_SYNC_TOLERANCE_NS - 1;
            }

            verify_segment(
                &skewed.header,
                &skewed.descriptors,
                &skewed.chunks,
                skewed.audio.as_ref(),
            )
            .expect("skew within tolerance");
        }

        #[test]
        fn verify_segment_rejects_audio_skew_beyond_tolerance() {
            let dims = FrameDimensions::new(4, 4);
            let audio_cfg = AudioEncoderConfig::default();
            let config = BaselineEncoderConfig {
                frame_dimensions: dims,
                frames_per_segment: 2,
                frame_duration_ns: 5_000_000,
                audio: Some(audio_cfg),
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config);
            let frames = vec![
                RawFrame::new(dims, vec![0x33; dims.pixel_count()]).expect("frame"),
                RawFrame::new(dims, vec![0x44; dims.pixel_count()]).expect("frame"),
            ];
            let channels = audio_cfg.channel_count();
            let samples_per_frame = audio_cfg.frame_samples as usize * channels;
            let pcm = vec![0i16; samples_per_frame * frames.len()];
            let segment = encoder
                .encode_segment(43, 5_000, 9, &frames, Some(&pcm))
                .expect("encode segment");

            let mut skewed = segment.clone();
            if let Some(audio) = skewed.audio.as_mut() {
                audio.frames[1].timestamp_ns += AUDIO_SYNC_TOLERANCE_NS + 1;
            }

            let err = verify_segment(
                &skewed.header,
                &skewed.descriptors,
                &skewed.chunks,
                skewed.audio.as_ref(),
            )
            .expect_err("skew beyond tolerance must fail");
            assert!(matches!(
                err,
                SegmentError::AudioTimestampMismatch(info) if info.index == 1
            ));
        }

        #[test]
        fn forward_dct_quantization_matches_golden() {
            let dct = forward_dct(&JPEG_SAMPLE_BLOCK);
            assert_eq!(dct, JPEG_SAMPLE_DCT_Q4);

            let quant = quantize_coeffs(&dct, 4);
            assert_eq!(quant, JPEG_SAMPLE_QUANT_Q4);

            let dequant = dequantize_coeffs(&quant, 4);
            assert_eq!(dequant, JPEG_SAMPLE_DEQUANT_Q4);

            let recon = inverse_dct(&dequant);
            assert_eq!(recon, JPEG_SAMPLE_RECON_Q4);
        }

        #[test]
        fn baseline_encoder_entropy_matches_golden_chunk() {
            let dims = FrameDimensions::new(8, 8);
            let config = BaselineEncoderConfig {
                frame_dimensions: dims,
                frame_duration_ns: 16_666_666,
                duration_ns: 16_666_666,
                quantizer: 4,
                frames_per_segment: 1,
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config);
            let frame_bytes: Vec<u8> = (0..dims.pixel_count())
                .map(|idx| (idx as u8).wrapping_mul(3).wrapping_add(17))
                .collect();
            let frame = RawFrame::new(dims, frame_bytes).expect("frame");
            let segment = encoder
                .encode_segment(42, 1_000, 7, &[frame], None)
                .expect("encode");

            assert_eq!(segment.chunks.len(), 1);
            assert_eq!(segment.chunks[0].as_slice(), GOLDEN_Q4_CHUNK);
        }

        #[test]
        fn decode_block_rle_rejects_overflow_run() {
            let dims = FrameDimensions::new(8, 8);
            let config = BaselineEncoderConfig {
                frame_dimensions: dims,
                frame_duration_ns: 16_666_666,
                duration_ns: 16_666_666,
                quantizer: 4,
                frames_per_segment: 1,
                ..BaselineEncoderConfig::default()
            };
            let mut encoder = BaselineEncoder::new(config);
            let frame_bytes: Vec<u8> = (0..dims.pixel_count())
                .map(|idx| (idx as u8).wrapping_mul(3).wrapping_add(17))
                .collect();
            let frame = RawFrame::new(dims, frame_bytes).expect("frame");
            let segment = encoder
                .encode_segment(7, 10_000, 3, &[frame], None)
                .expect("encode");

            let mut corrupted = segment.chunks[0].clone();
            let run_offset = FRAME_HEADER_LEN + 2;
            corrupted[run_offset] = 200;
            let mut offset = FRAME_HEADER_LEN;
            let mut prev_dc = 0i16;
            let err =
                decode_block_rle(&corrupted, &mut offset, &mut prev_dc, 0).expect_err("overflow");
            assert!(matches!(err, CodecError::RleOverflow(0)));
        }

        #[test]
        fn decode_block_rle_rejects_missing_end_of_block() {
            let mut payload = Vec::new();
            payload.extend_from_slice(&0i16.to_le_bytes());
            for _ in 0..(BLOCK_PIXELS - 1) {
                payload.push(0);
                payload.extend_from_slice(&0i16.to_le_bytes());
            }
            let mut offset = 0usize;
            let mut prev_dc = 0i16;
            let err = decode_block_rle(&payload, &mut offset, &mut prev_dc, 0)
                .expect_err("missing end-of-block should reject");
            assert!(matches!(err, CodecError::MissingEndOfBlock(0)));
        }

        #[test]
        fn decode_block_rle_rejects_truncated_stream() {
            let mut offset = 0usize;
            let mut prev_dc = 0i16;
            let err = decode_block_rle(&[0xAA], &mut offset, &mut prev_dc, 2)
                .expect_err("too short for dc diff");
            assert!(matches!(err, CodecError::TruncatedBlock(2)));

            let mut payload = Vec::new();
            payload.extend_from_slice(&0i16.to_le_bytes());
            payload.push(0x01);
            // missing coefficient value bytes
            let mut offset2 = 0usize;
            let mut prev_dc2 = 0i16;
            let err = decode_block_rle(&payload, &mut offset2, &mut prev_dc2, 4)
                .expect_err("missing run payload");
            assert!(matches!(err, CodecError::TruncatedBlock(4)));
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(64))]
            fn merkle_proof_roundtrip_holds(
                segment_number in any::<u64>(),
                content_key_id in any::<u64>(),
                payloads in pvec(pvec(any::<u8>(), 1..48), 1..6),
            ) {
                let chunk_ids: Vec<u16> = (0..payloads.len()).map(|idx| idx as u16).collect();
                let payload_refs: Vec<(u16, &[u8])> = chunk_ids
                    .iter()
                    .zip(payloads.iter())
                    .map(|(id, bytes)| (*id, bytes.as_slice()))
                    .collect();

                let commitments = chunk_commitments(segment_number, &payload_refs);
                let root = merkle_root(&commitments).expect("non-empty leaves produce root");

                for (idx, chunk_id) in chunk_ids.iter().enumerate() {
                    let proof = crate::streaming::chunk::merkle_proof(&commitments, idx, *chunk_id)
                        .expect("proof");
                    let leaf = commitments[idx];
                    prop_assert!(crate::streaming::chunk::verify_merkle_proof(
                        &leaf,
                        &proof,
                        &root
                    ));
                }

                let storage_hash = crate::streaming::chunk::storage_commitment(
                    segment_number,
                    content_key_id,
                    &root,
                    &chunk_ids,
                )
                .expect("storage commitment");
                let da_hash = crate::streaming::chunk::data_availability_root(
                    segment_number,
                    content_key_id,
                    &root,
                    &chunk_ids,
                )
                .expect("da root");
                prop_assert_ne!(storage_hash, da_hash);

                if chunk_ids.len() >= 2 {
                    let mut unsorted = chunk_ids.clone();
                    unsorted.swap(0, 1);
                    prop_assert!(matches!(
                        crate::streaming::chunk::storage_commitment(
                            segment_number,
                            content_key_id,
                            &root,
                            &unsorted
                        ),
                        Err(ChunkError::UnsortedChunkIds)
                    ));
                    prop_assert!(matches!(
                        crate::streaming::chunk::data_availability_root(
                            segment_number,
                            content_key_id,
                            &root,
                            &unsorted
                        ),
                        Err(ChunkError::UnsortedChunkIds)
                    ));
                }
            }
        }
    }
}

pub use codec::{
    BundleAnsTables, BundleTableError, default_bundle_tables, load_bundle_tables_from_toml,
};

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::{
        deserialize_from, json,
        streaming::codec::{
            BaselineEncoder, BaselineEncoderConfig, BaselineManifestParams, FrameDimensions,
            RawFrame,
        },
        to_bytes,
    };

    fn demo_hash(seed: u8) -> Hash {
        let mut bytes = [0u8; 32];
        bytes.fill(seed);
        bytes
    }

    fn demo_signature(seed: u8) -> Signature {
        let mut bytes = [0u8; 64];
        bytes.fill(seed);
        bytes
    }

    fn hex_encode(bytes: impl AsRef<[u8]>) -> String {
        const LUT: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(bytes.as_ref().len() * 2);
        for byte in bytes.as_ref() {
            out.push(char::from(LUT[(byte >> 4) as usize]));
            out.push(char::from(LUT[(byte & 0x0f) as usize]));
        }
        out
    }

    #[test]
    fn decode_from_slice_rejects_short_payloads() {
        let err = <SoranetRoute as crate::core::DecodeFromSlice>::decode_from_slice(&[])
            .expect_err("short soranet route");
        assert!(matches!(err, crate::core::Error::LengthMismatch));

        let err = <FeedbackHint as crate::core::DecodeFromSlice>::decode_from_slice(&[])
            .expect_err("short feedback hint");
        assert!(matches!(err, crate::core::Error::LengthMismatch));
    }

    #[test]
    fn decode_bundle_stream_simd_rejects_length_mismatch() {
        let tables = codec::default_bundle_tables();
        let mut stream = Vec::new();
        stream.extend_from_slice(b"BR4\x01");
        stream.extend_from_slice(&1u32.to_le_bytes());
        stream.extend_from_slice(&0u32.to_le_bytes());
        stream.extend_from_slice(&0u32.to_le_bytes());
        stream.extend_from_slice(&0u32.to_le_bytes());
        let bundles: Vec<codec::BundleRecord> = Vec::new();
        let err =
            codec::decode_bundle_stream(&stream, &bundles, tables.as_ref()).expect_err("bad len");
        assert!(matches!(err, codec::BundleDecodeError::LengthMismatch));
    }

    #[test]
    fn transparent_wrappers_roundtrip_individually() {
        fn roundtrip<T>(value: T) -> T
        where
            T: NoritoSerialize
                + for<'de> NoritoDeserialize<'de>
                + Copy
                + PartialEq
                + std::fmt::Debug,
        {
            let encoded = to_bytes(&value).expect("serialize wrapper");
            deserialize_from(encoded.as_slice()).expect("deserialize wrapper")
        }

        let profile = ProfileId::UHD_AI;
        assert_eq!(roundtrip(profile), profile);

        let capability_flags = CapabilityFlags::from_bits(
            CapabilityFlags::FEATURE_FEEDBACK_HINTS | CapabilityFlags::FEATURE_SM_TRANSACTIONS,
        );
        assert_eq!(roundtrip(capability_flags), capability_flags);

        let privacy = PrivacyCapabilities::from_bits(0b1010);
        assert_eq!(roundtrip(privacy), privacy);

        let suites = HpkeSuiteMask::KYBER1024;
        assert_eq!(roundtrip(suites), suites);

        let channel = SoranetChannelId::new(demo_hash(0x90));
        assert_eq!(roundtrip(channel), channel);

        let ticket_caps =
            TicketCapabilities::from_bits(TicketCapabilities::LIVE | TicketCapabilities::HDR);
        assert_eq!(roundtrip(ticket_caps), ticket_caps);
    }

    #[test]
    fn manifest_roundtrip() {
        let chunk = ChunkDescriptor {
            chunk_id: 0,
            offset: 0,
            length: 1024,
            commitment: demo_hash(3),
            parity: false,
        };
        let manifest = ManifestV1 {
            stream_id: demo_hash(1),
            protocol_version: 1,
            segment_number: 42,
            published_at: 1_694_000_000,
            profile: ProfileId::BASELINE,
            entropy_mode: crate::streaming::EntropyMode::RansBundled,
            entropy_tables_checksum: None,
            da_endpoint: "/ip4/127.0.0.1/udp/9000/quic".into(),
            chunk_root: demo_hash(2),
            content_key_id: 7,
            nonce_salt: demo_hash(4),
            chunk_descriptors: vec![chunk],
            audio_summary: None,
            transport_capabilities_hash: demo_hash(11),
            encryption_suite: EncryptionSuite::X25519ChaCha20Poly1305(demo_hash(5)),
            fec_suite: FecScheme::Rs12_10,
            privacy_routes: vec![PrivacyRoute {
                route_id: demo_hash(6),
                entry: PrivacyRelay {
                    relay_id: demo_hash(7),
                    endpoint: "/dns/entry.example/quic".into(),
                    key_fingerprint: demo_hash(8),
                    capabilities: PrivacyCapabilities::from_bits(0b001),
                },
                exit: PrivacyRelay {
                    relay_id: demo_hash(9),
                    endpoint: "/dns/exit.example/quic".into(),
                    key_fingerprint: demo_hash(10),
                    capabilities: PrivacyCapabilities::from_bits(0b010),
                },
                ticket_entry: vec![1, 2, 3],
                ticket_exit: vec![4, 5, 6],
                expiry_segment: 100,
                soranet: None,
            }],
            neural_bundle: Some(NeuralBundle {
                bundle_id: "bundle-v1".into(),
                weights_sha256: demo_hash(11),
                activation_scale: vec![123, 234],
                bias: vec![1000, -42],
                metadata_signature: demo_signature(12),
                metal_shader_sha256: Some(demo_hash(13)),
                cuda_ptx_sha256: None,
            }),
            public_metadata: StreamMetadata {
                title: "Test Stream".into(),
                description: Some("Demo manifest".into()),
                access_policy_id: Some(demo_hash(14)),
                tags: vec!["demo".into(), "nsc".into()],
            },
            capabilities: CapabilityFlags::from_bits(0b1_0101),
            signature: demo_signature(15),
        };

        let encoded = to_bytes(&manifest).expect("serialize");
        let decoded: ManifestV1 = deserialize_from(encoded.as_slice()).expect("deserialize");
        assert_eq!(decoded, manifest);
    }

    #[test]
    fn privacy_route_with_soranet_metadata_roundtrip() {
        let route = PrivacyRoute {
            route_id: demo_hash(0x21),
            entry: PrivacyRelay {
                relay_id: demo_hash(0x22),
                endpoint: "/dns/soranet.entry/quic".into(),
                key_fingerprint: demo_hash(0x23),
                capabilities: PrivacyCapabilities::from_bits(0b001),
            },
            exit: PrivacyRelay {
                relay_id: demo_hash(0x24),
                endpoint: "/dns/soranet.exit/quic".into(),
                key_fingerprint: demo_hash(0x25),
                capabilities: PrivacyCapabilities::from_bits(0b010),
            },
            ticket_entry: vec![0x10, 0x11, 0x12],
            ticket_exit: vec![0x20, 0x21, 0x22],
            expiry_segment: 256,
            soranet: Some(SoranetRoute {
                channel_id: SoranetChannelId::new(demo_hash(0x26)),
                exit_multiaddr: "/dns/torii.exit.example/tcp/8080".into(),
                padding_budget_ms: Some(12),
                access_kind: SoranetAccessKind::Authenticated,
                stream_tag: SoranetStreamTag::NoritoStream,
            }),
        };

        let encoded = to_bytes(&route).expect("serialize privacy route");
        let decoded: PrivacyRoute =
            deserialize_from(encoded.as_slice()).expect("deserialize privacy route");
        assert_eq!(decoded, route);
        let soranet = decoded.soranet.expect("soranet metadata present");
        assert_eq!(
            soranet.channel_id.as_bytes(),
            &demo_hash(0x26),
            "channel id must roundtrip"
        );
    }

    #[test]
    fn soranet_stream_tag_default_is_norito() {
        assert_eq!(SoranetStreamTag::default(), SoranetStreamTag::NoritoStream);
    }

    #[test]
    fn receiver_report_with_sync_diagnostics_roundtrip() {
        let diagnostics = SyncDiagnostics {
            window_ms: 500,
            samples: 96,
            avg_audio_jitter_ms: 4,
            max_audio_jitter_ms: 9,
            avg_av_drift_ms: -3,
            max_av_drift_ms: 11,
            ewma_av_drift_ms: -2,
            violation_count: 1,
        };
        let report = ReceiverReport {
            stream_id: demo_hash(0xA0),
            latest_segment: 128,
            layer_mask: 0b101,
            measured_throughput_kbps: 2_400,
            rtt_ms: 37,
            loss_percent_x100: 250,
            decoder_buffer_ms: 180,
            active_resolution: Resolution::R1080p,
            hdr_active: true,
            ecn_ce_count: 4,
            jitter_ms: 7,
            delivered_sequence: 9_001,
            parity_applied: 2,
            fec_budget: 3,
            sync_diagnostics: Some(diagnostics),
        };

        let encoded = to_bytes(&report).expect("serialize receiver report");
        let decoded: ReceiverReport =
            deserialize_from(encoded.as_slice()).expect("deserialize receiver report");
        assert_eq!(decoded, report);
    }

    #[test]
    fn streaming_ticket_roundtrip() {
        let capabilities = TicketCapabilities::from_bits(
            TicketCapabilities::LIVE | TicketCapabilities::HDR | TicketCapabilities::SPATIAL_AUDIO,
        );
        let policy = TicketPolicy {
            max_relays: 4,
            allowed_regions: vec!["us".into(), "jp".into()],
            max_bandwidth_kbps: Some(15_000),
        };
        let ticket = StreamingTicket {
            ticket_id: demo_hash(0x44),
            owner: "bob@streaming".to_owned(),
            dsid: 7,
            lane_id: 5,
            settlement_bucket: 2_048,
            start_slot: 21_000,
            expire_slot: 24_000,
            prepaid_teu: 120_000,
            chunk_teu: 64,
            fanout_quota: 12,
            key_commitment: demo_hash(0x55),
            nonce: 42,
            contract_sig: demo_signature(0x66),
            commitment: demo_hash(0x77),
            nullifier: demo_hash(0x88),
            proof_id: demo_hash(0x99),
            issued_at: 1_701_234_567,
            expires_at: 1_701_834_567,
            policy: Some(policy),
            capabilities,
        };

        let encoded = to_bytes(&ticket).expect("serialize ticket");
        let decoded: StreamingTicket =
            deserialize_from(encoded.as_slice()).expect("deserialize ticket");
        assert_eq!(decoded, ticket);
        assert!(decoded.capabilities.contains(TicketCapabilities::HDR));
        assert!(!decoded.capabilities.contains(TicketCapabilities::VOD));
    }

    #[test]
    fn ticket_revocation_roundtrip() {
        let revocation = TicketRevocation {
            ticket_id: demo_hash(0xAA),
            nullifier: demo_hash(0xBB),
            reason_code: 17,
            revocation_signature: demo_signature(0xCC),
        };
        let encoded = to_bytes(&revocation).expect("serialize ticket revocation");
        let decoded: TicketRevocation =
            deserialize_from(encoded.as_slice()).expect("deserialize ticket revocation");
        assert_eq!(decoded, revocation);
    }

    #[test]
    fn control_frame_roundtrip() {
        let frame = ControlFrame::KeyUpdate(KeyUpdate {
            session_id: demo_hash(20),
            suite: EncryptionSuite::Kyber768XChaCha20Poly1305(demo_hash(21)),
            protocol_version: 1,
            pub_ephemeral: vec![0u8; 32],
            key_counter: 5,
            signature: demo_signature(22),
        });

        let encoded = to_bytes(&frame).expect("serialize");
        let decoded: ControlFrame = deserialize_from(encoded.as_slice()).expect("deserialize");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn telemetry_roundtrip() {
        let event = TelemetryEvent::Security(TelemetrySecurityStats {
            suite: EncryptionSuite::X25519ChaCha20Poly1305(demo_hash(30)),
            rekeys: 3,
            gck_rotations: 1,
            last_content_key_id: Some(42),
            last_content_key_valid_from: Some(1_700_123_456),
        });

        let encoded = to_bytes(&event).expect("serialize");
        let decoded: TelemetryEvent = deserialize_from(encoded.as_slice()).expect("deserialize");
        assert_eq!(decoded, event);
    }

    #[test]
    fn segment_header_roundtrip() {
        let header = SegmentHeader {
            segment_number: 77,
            profile: ProfileId::UHD_MAIN,
            entropy_mode: crate::streaming::EntropyMode::RansBundled,
            entropy_tables_checksum: None,
            encryption_suite: EncryptionSuite::X25519ChaCha20Poly1305(demo_hash(40)),
            layer_bitmap: 0b11,
            chunk_merkle_root: demo_hash(41),
            chunk_count: 12,
            timeline_start_ns: 1000,
            duration_ns: 250_000_000,
            feedback_hint: FeedbackHint {
                layer_hints: vec![LayerFeedback {
                    layer_id: 0,
                    min_target_kbps: 1200,
                    max_target_kbps: 2400,
                    storage_hint: Some(StorageClass::Permanent),
                }],
                report_interval_ms: Some(750),
                fec: Some(FecParameters {
                    scheme: FecScheme::Rs18_14,
                    window_step: Some(5),
                    parity_symbols: Some(4),
                }),
            },
            content_key_id: 11,
            nonce_salt: demo_hash(42),
            storage_class: StorageClass::Ephemeral,
            audio_summary: None,
            bundle_acceleration: BundleAcceleration::None,
        };

        let encoded = to_bytes(&header).expect("serialize");
        let decoded: SegmentHeader = deserialize_from(encoded.as_slice()).expect("deserialize");
        assert_eq!(decoded, header);
    }

    #[test]
    fn feedback_hint_frame_roundtrip() {
        let frame = FeedbackHintFrame {
            stream_id: demo_hash(50),
            loss_ewma_q16: 0x0001_8000,
            latency_gradient_q16: -0x0000_4000,
            observed_rtt_ms: 42,
            report_interval_ms: 250,
            parity_chunks: 3,
        };
        let encoded = to_bytes(&frame).expect("serialize");
        let decoded: FeedbackHintFrame = deserialize_from(encoded.as_slice()).expect("deserialize");
        assert_eq!(decoded, frame);
    }

    #[test]
    #[ignore]
    fn dump_baseline_streaming_snapshot() {
        use crate::streaming::chunk;

        let dims = FrameDimensions::new(8, 8);
        let frame_duration_ns = 25_000_000u32;
        let luma = vec![0x55; dims.pixel_count()];
        let frames = vec![
            RawFrame::new(dims, luma.clone()).expect("frame 0"),
            RawFrame::new(dims, luma).expect("frame 1"),
        ];

        let config = BaselineEncoderConfig {
            frame_dimensions: dims,
            frame_duration_ns,
            duration_ns: frame_duration_ns.saturating_mul(frames.len() as u32),
            quantizer: 0,
            ..BaselineEncoderConfig::default()
        };

        let mut encoder = BaselineEncoder::new(config.clone());
        let segment = encoder
            .encode_segment(5, 1_000_000, 3, &frames, None)
            .expect("encode baseline segment");

        let params = BaselineManifestParams {
            stream_id: demo_hash(0x31),
            protocol_version: 1,
            published_at: 1_703_000_000,
            da_endpoint: "/ip4/127.0.0.1/udp/9100/quic".into(),
            privacy_routes: Vec::new(),
            public_metadata: StreamMetadata {
                title: "NSC Baseline Vector".into(),
                description: Some("Canonical manifest for Norito streaming harness.".into()),
                access_policy_id: None,
                tags: vec!["nsc".into(), "baseline".into()],
            },
            capabilities: CapabilityFlags::from_bits(0b0111),
            signature: demo_signature(0x41),
            fec_suite: FecScheme::Rs12_10,
            neural_bundle: None,
            transport_capabilities_hash: [0u8; 32],
        };

        let manifest = segment.build_manifest(params);
        let chunk_refs: Vec<(u16, &[u8])> = segment
            .descriptors
            .iter()
            .zip(segment.chunks.iter())
            .map(|(descriptor, chunk)| (descriptor.chunk_id, chunk.as_slice()))
            .collect();
        let chunk_commitments =
            chunk::chunk_commitments(segment.header.segment_number, &chunk_refs);
        let chunk_ids: Vec<u16> = segment
            .descriptors
            .iter()
            .map(|descriptor| descriptor.chunk_id)
            .collect();
        let storage_commitment = chunk::storage_commitment(
            segment.header.segment_number,
            segment.header.content_key_id,
            &segment.header.chunk_merkle_root,
            &chunk_ids,
        )
        .expect("storage commitment");
        let da_root = chunk::data_availability_root(
            segment.header.segment_number,
            segment.header.content_key_id,
            &segment.header.chunk_merkle_root,
            &chunk_ids,
        )
        .expect("da root");
        let manifest_bytes = to_bytes(&manifest).expect("serialize manifest");

        let capabilities = TicketCapabilities::from_bits(
            TicketCapabilities::LIVE | TicketCapabilities::HDR | TicketCapabilities::SPATIAL_AUDIO,
        );
        let ticket_policy = TicketPolicy {
            max_relays: 4,
            allowed_regions: vec!["us".into(), "jp".into()],
            max_bandwidth_kbps: Some(15_000),
        };
        let ticket = StreamingTicket {
            ticket_id: demo_hash(0x44),
            owner: "viewer@stream.test".into(),
            dsid: 7,
            lane_id: 5,
            settlement_bucket: 2_048,
            start_slot: 21_000,
            expire_slot: 24_000,
            prepaid_teu: 120_000,
            chunk_teu: 64,
            fanout_quota: 12,
            key_commitment: demo_hash(0x55),
            nonce: 42,
            contract_sig: demo_signature(0x66),
            commitment: demo_hash(0x77),
            nullifier: demo_hash(0x88),
            proof_id: demo_hash(0x99),
            issued_at: 1_701_234_567,
            expires_at: 1_701_834_567,
            policy: Some(ticket_policy),
            capabilities,
        };
        let ticket_revocation = TicketRevocation {
            ticket_id: demo_hash(0xAA),
            nullifier: demo_hash(0xBB),
            reason_code: 17,
            revocation_signature: demo_signature(0xCC),
        };

        let mut snapshot = norito::json::Map::new();
        snapshot.insert(
            "manifest_template_hex".into(),
            json::to_value(&hex_encode(&manifest_bytes)).expect("json"),
        );
        snapshot.insert(
            "chunk_root".into(),
            json::to_value(&hex_encode(segment.header.chunk_merkle_root)).expect("json"),
        );
        snapshot.insert(
            "chunk_commitments".into(),
            json::to_value(&chunk_commitments.iter().map(hex_encode).collect::<Vec<_>>())
                .expect("json"),
        );
        snapshot.insert(
            "chunk_payloads".into(),
            json::to_value(&segment.chunks.iter().map(hex_encode).collect::<Vec<_>>())
                .expect("json"),
        );
        snapshot.insert(
            "storage_commitment".into(),
            json::to_value(&hex_encode(storage_commitment)).expect("json"),
        );
        snapshot.insert(
            "da_root".into(),
            json::to_value(&hex_encode(da_root)).expect("json"),
        );
        snapshot.insert(
            "ticket".into(),
            norito::json::Value::from(format!("{ticket:?}")),
        );
        snapshot.insert(
            "ticket_revocation".into(),
            norito::json::Value::from(format!("{ticket_revocation:?}")),
        );

        let json_value = norito::json::Value::Object(snapshot);
        let snapshot_json = json::to_string_pretty(&json_value).expect("json encode");
        println!("{snapshot_json}");
    }

    #[test]
    fn bundle_tables_enforce_max_width() {
        let precision_bits = 12;
        let frequencies_2 = vec![1024u16; 4];
        let cumulative_2 = (0..=4).map(|idx| (idx * 1024) as u32).collect::<Vec<_>>();
        let frequencies_3 = vec![512u16; 8];
        let cumulative_3 = (0..=8).map(|idx| (idx * 512) as u32).collect::<Vec<_>>();
        let body = RansTablesBodyV1 {
            seed: 9,
            bundle_width: 3,
            groups: vec![
                RansGroupTableV1 {
                    width_bits: 2,
                    group_size: 4,
                    precision_bits,
                    frequencies: frequencies_2,
                    cumulative: cumulative_2,
                },
                RansGroupTableV1 {
                    width_bits: 3,
                    group_size: 8,
                    precision_bits,
                    frequencies: frequencies_3,
                    cumulative: cumulative_3,
                },
            ],
        };
        let checksum = {
            let bytes = to_bytes(&body).expect("encode tables");
            let digest = Sha256::digest(bytes);
            let mut out = [0u8; 32];
            out.copy_from_slice(&digest);
            out
        };
        let payload = RansTablesV1 {
            version: 1,
            generated_at: 0,
            generator_commit: "test".into(),
            checksum_sha256: checksum,
            body,
        };
        let signed = SignedRansTablesV1 {
            payload,
            signature: None,
        };
        let tables = BundleAnsTables::from_signed_for_tests(&signed).expect("load tables");
        assert_eq!(tables.max_width(), 3);
        assert_eq!(
            tables.freq_len_for_bits_for_tests(2),
            Some(4),
            "2-bit table should have 4 symbols"
        );
        assert_eq!(
            tables.freq_len_for_bits_for_tests(3),
            Some(8),
            "3-bit table should have 8 symbols"
        );
        assert!(
            tables.freq_len_for_bits_for_tests(4).is_none(),
            "should reject widths above bundle limit"
        );
    }

    fn write_temp_tables_toml(contents: &str) -> std::path::PathBuf {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("current time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "norito-bundle-tables-{pid}-{suffix}.toml",
            pid = std::process::id()
        ));
        fs::write(&path, contents).expect("write temp tables toml");
        path
    }

    #[test]
    fn load_bundle_tables_accepts_mangled_payload_string() {
        let payload = r#"{"version""1","generated_at""1765012527","generator_commit""7fa1c4c20a921ae51e6a5fd575cfe8ee06d14877","checksum_sha256""430943A6D4E8BB34DE9A39879C39FAA5814B5D1FE3D13FE43DEFBA1F2C1741F2","body":{"bundle_width""3","seed""0","groups":[{"width_bits""2","group_size""4","precision_bits""12","frequencies":["542","1113","1011","1430"],"cumulative":["0","542","1655","2666","4096"]},{"width_bits""3","group_size""8","precision_bits""12","frequencies":["280","262","672","441","818","193","692","738"],"cumulative":["0","280","542","1214","1655","2473","2666","3358","4096"]}]}}"#;
        let toml = format!("payload = '''{payload}'''\nsignature = ''\n");
        let path = write_temp_tables_toml(&toml);

        let tables = load_bundle_tables_from_toml(&path).expect("decode mangled payload string");
        std::fs::remove_file(&path).ok();

        assert_eq!(tables.max_width(), 3);
        assert_eq!(
            tables.freq_len_for_bits_for_tests(3),
            Some(8),
            "expected 3-bit group frequencies from payload"
        );
    }

    #[test]
    fn load_bundle_tables_accepts_repo_fixture() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../codec/rans/tables/rans_seed0.toml");
        let tables = load_bundle_tables_from_toml(&path)
            .unwrap_or_else(|err| panic!("failed to load {}: {err}", path.display()));
        assert!(
            tables.max_width() >= 2,
            "expected deterministic tables fixture to expose bundled widths"
        );
    }
}
