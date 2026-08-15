#![allow(clippy::useless_let_if_seq)]
use crate::sorafs::pin_registry::StorageClass;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_primitives::numeric::{RoundingMode, XorQuantity};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sorafs_manifest::deal::BASIS_POINTS_PER_UNIT;
use std::ops::{Deref, DerefMut};
use thiserror::Error;
/// Blake3-based digest used across DA ingest structures (blob identifiers, manifests, tickets).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema, Default,
)]
#[repr(transparent)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct BlobDigest(
    #[cfg_attr(
        feature = "json",
        norito(
            with = "crate::json_helpers::fixed_bytes",
            bounded_with = "crate::json_helpers::fixed_bytes::serialize_bounded"
        )
    )]
    pub [u8; 32],
);
impl BlobDigest {
    /// Construct a new digest wrapper.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Returns `true` when all digest bytes are zeroed.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|byte| *byte == 0)
    }
    /// Construct a digest from a Blake3 hash.
    #[must_use]
    pub fn from_hash(hash: blake3::Hash) -> Self {
        Self(hash.into())
    }
    /// Access the raw digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl Deref for BlobDigest {
    type Target = [u8; 32];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for BlobDigest {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl AsRef<[u8; 32]> for BlobDigest {
    fn as_ref(&self) -> &[u8; 32] {
        &self.0
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for BlobDigest {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = bytes;
        let start_len = cursor.len();
        let value = Self::decode(&mut cursor)?;
        let consumed = start_len - cursor.len();
        Ok((value, consumed))
    }
}
/// Chunk-level commitment digest.
pub type ChunkDigest = BlobDigest;
/// Identifier referencing a storage ticket issued by the `SoraFS` orchestrator.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema, Default,
)]
#[repr(transparent)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct StorageTicketId(
    #[cfg_attr(
        feature = "json",
        norito(
            with = "crate::json_helpers::fixed_bytes",
            bounded_with = "crate::json_helpers::fixed_bytes::serialize_bounded"
        )
    )]
    pub [u8; 32],
);
impl StorageTicketId {
    /// Construct a new storage ticket identifier.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Construct a ticket identifier from a Blake3 hash.
    #[must_use]
    pub fn from_hash(hash: blake3::Hash) -> Self {
        Self(hash.into())
    }
    /// Access the raw identifier bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl Deref for StorageTicketId {
    type Target = [u8; 32];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for StorageTicketId {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl AsRef<[u8; 32]> for StorageTicketId {
    fn as_ref(&self) -> &[u8; 32] {
        &self.0
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for StorageTicketId {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = bytes;
        let start_len = cursor.len();
        let value = Self::decode(&mut cursor)?;
        let consumed = start_len - cursor.len();
        Ok((value, consumed))
    }
}
/// Semantic classification for an incoming DA blob.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "class", content = "value")]
pub enum BlobClass {
    /// Taikai broadcast segment.
    #[default]
    TaikaiSegment,
    /// Nexus lane sidecar / rollup payload.
    NexusLaneSidecar,
    /// Governance artefact (e.g., signed minutes, charter updates).
    GovernanceArtifact,
    /// Future-proof custom class reserved for governance-approved extensions.
    Custom(u16),
}
/// Codec label describing the blob payload.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default)]
#[repr(transparent)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct BlobCodec(pub String);
impl BlobCodec {
    /// Construct a codec label.
    #[must_use]
    pub fn new(codec: impl Into<String>) -> Self {
        Self(codec.into())
    }
}
/// Compression applied to the submitted payload.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default, Hash,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "value"))]
pub enum Compression {
    /// Payload is provided as-is.
    #[default]
    Identity,
    /// Payload compressed with the gzip format (RFC 1952).
    Gzip,
    /// Payload compressed with zlib-wrapped DEFLATE (RFC 1950).
    Deflate,
    /// Payload compressed with Zstandard.
    Zstd,
}
/// Governance tag tying a blob to a retention or policy decision.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Default, Hash,
)]
#[repr(transparent)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GovernanceTag(pub String);
impl GovernanceTag {
    /// Construct a governance tag wrapper.
    #[must_use]
    pub fn new(tag: impl Into<String>) -> Self {
        Self(tag.into())
    }
}
/// Forward-error-correction schemes supported by the DA layer.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default, Hash, PartialOrd, Ord,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "scheme", content = "value")]
pub enum FecScheme {
    /// Mandatory RS 12/10 configuration.
    #[default]
    Rs12_10,
    /// Sliding-window RS 14/10 variant.
    RsWin14_10,
    /// Sliding-window RS 18/14 variant.
    Rs18_14,
    /// Governance-approved custom profile identified by numeric code.
    Custom(u16),
}
impl From<FecScheme> for norito::streaming::FecScheme {
    fn from(value: FecScheme) -> Self {
        match value {
            FecScheme::RsWin14_10 => norito::streaming::FecScheme::RsWin14_10,
            FecScheme::Rs18_14 => norito::streaming::FecScheme::Rs18_14,
            _ => norito::streaming::FecScheme::Rs12_10,
        }
    }
}
impl From<norito::streaming::FecScheme> for FecScheme {
    fn from(value: norito::streaming::FecScheme) -> Self {
        match value {
            norito::streaming::FecScheme::Rs12_10 => FecScheme::Rs12_10,
            norito::streaming::FecScheme::RsWin14_10 => FecScheme::RsWin14_10,
            norito::streaming::FecScheme::Rs18_14 => FecScheme::Rs18_14,
        }
    }
}
/// Erasure coding parameters applied during chunking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ErasureProfile {
    /// Number of data shards in each erasure set.
    pub data_shards: u16,
    /// Number of parity shards.
    pub parity_shards: u16,
    /// Number of parity stripes across rows (column-parity stripes).
    #[norito(default)]
    #[norito(skip_serializing_if = "is_zero_u16")]
    pub row_parity_stripes: u16,
    /// Chunk alignment (chunks per availability slice).
    pub chunk_alignment: u16,
    /// FEC scheme describing the encoding.
    pub fec_scheme: FecScheme,
}
impl Default for ErasureProfile {
    fn default() -> Self {
        Self {
            data_shards: 10,
            parity_shards: 4,
            row_parity_stripes: 0,
            chunk_alignment: 10,
            fec_scheme: FecScheme::Rs12_10,
        }
    }
}
// Norito's `skip_serializing_if` predicates take a reference to the field value.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_u16(value: &u16) -> bool {
    *value == 0
}
/// Retention policy negotiated for a DA blob.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct RetentionPolicy {
    /// Hot-tier retention period in seconds.
    pub hot_retention_secs: u64,
    /// Cold-tier retention period in seconds.
    pub cold_retention_secs: u64,
    /// Required replica count for the blob.
    pub required_replicas: u16,
    /// Storage class requested for the primary replicas.
    pub storage_class: StorageClass,
    /// Governance tag anchoring the policy decision.
    pub governance_tag: GovernanceTag,
}
impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            hot_retention_secs: 7 * 24 * 60 * 60,
            cold_retention_secs: 90 * 24 * 60 * 60,
            required_replicas: 3,
            storage_class: StorageClass::Hot,
            governance_tag: GovernanceTag::new("da.default"),
        }
    }
}
/// Optional metadata entries supplied by submitters.
#[derive(Clone, Debug, Default, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ExtraMetadata {
    /// Metadata key-value pairs.
    pub items: Vec<MetadataEntry>,
}
impl ExtraMetadata {
    /// Returns `true` when no metadata entries are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
/// Encryption algorithm applied to a metadata entry.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "cipher", content = "params"))]
pub enum MetadataEncryption {
    /// No encryption applied; `value` is stored as-is.
    #[default]
    None,
    /// `ChaCha20Poly1305` envelope with optional metadata (e.g., key labels).
    ChaCha20Poly1305(#[norito(default)] MetadataCipherEnvelope),
}
impl MetadataEncryption {
    /// Build a ChaCha20-Poly1305 envelope with an optional key label.
    #[must_use]
    pub fn chacha20poly1305_with_label(label: Option<impl Into<String>>) -> Self {
        Self::ChaCha20Poly1305(MetadataCipherEnvelope::with_label(label))
    }
}
/// Additional envelope metadata associated with an encrypted entry.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MetadataCipherEnvelope {
    /// Optional label identifying the key used for encryption.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub key_label: Option<String>,
}
impl MetadataCipherEnvelope {
    /// Create an envelope with the provided key label.
    #[must_use]
    pub fn with_label<L>(label: Option<L>) -> Self
    where
        L: Into<String>,
    {
        Self {
            key_label: label.map(Into::into),
        }
    }
}
/// Single metadata entry stored alongside the blob.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct MetadataEntry {
    /// Metadata key (UTF-8, governance-approved).
    pub key: String,
    /// Raw metadata value bytes (Norito or application-specific encoding).
    #[cfg_attr(
        feature = "json",
        norito(
            with = "crate::json_helpers::base64_vec",
            bounded_with = "crate::json_helpers::base64_vec::serialize_bounded"
        )
    )]
    pub value: Vec<u8>,
    /// Visibility scope for the entry.
    pub visibility: MetadataVisibility,
    /// Encryption applied to the value (defaults to `None`).
    #[norito(default)]
    pub encryption: MetadataEncryption,
}
impl MetadataEntry {
    /// Construct a metadata entry.
    #[must_use]
    pub fn new(key: impl Into<String>, value: Vec<u8>, visibility: MetadataVisibility) -> Self {
        Self::with_encryption(key, value, visibility, MetadataEncryption::None)
    }
    /// Construct a metadata entry with explicit encryption metadata.
    #[must_use]
    pub fn with_encryption(
        key: impl Into<String>,
        value: Vec<u8>,
        visibility: MetadataVisibility,
        encryption: MetadataEncryption,
    ) -> Self {
        Self {
            key: key.into(),
            value,
            visibility,
            encryption,
        }
    }
}
/// Visibility scope for a metadata entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "visibility", content = "value")]
pub enum MetadataVisibility {
    /// Entry is visible to all consumers.
    #[default]
    Public,
    /// Entry is restricted to governance/oversight entities.
    GovernanceOnly,
}
/// Schema version for [`DaRentPolicyV1`].
pub const DA_RENT_POLICY_VERSION_V1: u8 = 1;
/// Maximum decimal scale accepted for configured DA rent rates.
///
/// DA rates intentionally retain micro-XOR precision so basis-point splits are deterministic at the
/// policy's accounting granularity. Quoted values still use [`XorQuantity`] at every boundary,
/// preventing other DA amounts from exceeding XOR's independent nine-decimal ledger limit.
const DA_RENT_RATE_SCALE: u32 = 6;
/// Rent and incentive policy for DA submissions (see roadmap task DA-7).
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaRentPolicyV1 {
    /// Schema version (`DA_RENT_POLICY_VERSION_V1`).
    pub version: u8,
    /// Base XOR amount charged per GiB-month of storage.
    pub base_rate_per_gib_month: XorQuantity,
    /// Portion of the rent routed to the protocol reserve (basis points).
    pub protocol_reserve_bps: u16,
    /// Bonus awarded for each successful PDP execution (basis points).
    pub pdp_bonus_bps: u16,
    /// Bonus awarded for each successful `PoTR` execution (basis points).
    pub potr_bonus_bps: u16,
    /// XOR credit per GiB of egress served to fetchers.
    pub egress_credit_per_gib: XorQuantity,
}
impl Default for DaRentPolicyV1 {
    fn default() -> Self {
        Self {
            version: DA_RENT_POLICY_VERSION_V1,
            // 0.25 XOR per GiB-month baseline.
            base_rate_per_gib_month: "0.25".parse().expect("default is canonical"),
            protocol_reserve_bps: 2_000, // 20%
            pdp_bonus_bps: 500,          // 5%
            potr_bonus_bps: 250,         // 2.5%
            // 0.0015 XOR per GiB egress credit.
            egress_credit_per_gib: "0.0015".parse().expect("default is canonical"),
        }
    }
}
impl DaRentPolicyV1 {
    /// Construct a policy from exact XOR rates and basis-point parameters.
    #[must_use]
    pub fn from_components(
        base_rate_per_gib_month: XorQuantity,
        protocol_reserve_bps: u16,
        pdp_bonus_bps: u16,
        potr_bonus_bps: u16,
        egress_credit_per_gib: XorQuantity,
    ) -> Self {
        Self {
            version: DA_RENT_POLICY_VERSION_V1,
            base_rate_per_gib_month,
            protocol_reserve_bps,
            pdp_bonus_bps,
            potr_bonus_bps,
            egress_credit_per_gib,
        }
    }
    /// Validate the policy parameters.
    ///
    /// # Errors
    ///
    /// Returns [`DaRentError`] when version, ratio, or rate invariants fail.
    pub fn validate(&self) -> Result<(), DaRentError> {
        if self.version != DA_RENT_POLICY_VERSION_V1 {
            return Err(DaRentError::UnsupportedVersion {
                found: self.version,
            });
        }
        Self::validate_ratio(self.protocol_reserve_bps, RentRatioField::ProtocolReserve)?;
        Self::validate_ratio(self.pdp_bonus_bps, RentRatioField::PdpBonus)?;
        Self::validate_ratio(self.potr_bonus_bps, RentRatioField::PotrBonus)?;
        if self.base_rate_per_gib_month.is_zero() {
            return Err(DaRentError::ZeroRate);
        }
        if self.base_rate_per_gib_month.as_quantity().scale() > DA_RENT_RATE_SCALE
            || self.egress_credit_per_gib.as_quantity().scale() > DA_RENT_RATE_SCALE
        {
            return Err(DaRentError::PrecisionTooFine);
        }
        Ok(())
    }
    fn validate_ratio(value: u16, field: RentRatioField) -> Result<(), DaRentError> {
        if u32::from(value) > u32::from(BASIS_POINTS_PER_UNIT) {
            return Err(DaRentError::InvalidRatio {
                field,
                basis_points: value,
            });
        }
        Ok(())
    }
    /// Quote the rent and incentive components for the provided usage.
    ///
    /// # Errors
    ///
    /// Returns [`DaRentError`] when the policy is invalid or when the usage
    /// inputs lead to arithmetic overflows/underflows.
    pub fn quote(&self, gib: u64, months: u32) -> Result<DaRentQuote, DaRentError> {
        self.validate()?;
        if gib == 0 {
            return Err(DaRentError::ZeroUsage);
        }
        if months == 0 {
            return Err(DaRentError::ZeroDuration);
        }
        let units = u128::from(gib) * u128::from(months);
        let base_rent = self
            .base_rate_per_gib_month
            .checked_mul_u128(units)
            .map_err(|_| DaRentError::Overflow)?;
        let protocol_reserve = apply_basis_points(&base_rent, self.protocol_reserve_bps)?;
        let provider_reward = base_rent
            .checked_sub(&protocol_reserve)
            .map_err(|_| DaRentError::Overflow)?;
        let pdp_bonus = apply_basis_points(&base_rent, self.pdp_bonus_bps)?;
        let potr_bonus = apply_basis_points(&base_rent, self.potr_bonus_bps)?;
        Ok(DaRentQuote {
            base_rent,
            protocol_reserve,
            provider_reward,
            pdp_bonus,
            potr_bonus,
            egress_credit_per_gib: self.egress_credit_per_gib.clone(),
        })
    }
}
fn apply_basis_points(amount: &XorQuantity, basis_points: u16) -> Result<XorQuantity, DaRentError> {
    amount
        .checked_mul_ratio_round(
            u64::from(basis_points),
            core::num::NonZeroU64::new(u64::from(BASIS_POINTS_PER_UNIT))
                .expect("basis-point denominator is non-zero"),
            DA_RENT_RATE_SCALE,
            RoundingMode::TowardZero,
        )
        .map_err(|_| DaRentError::Overflow)
}
/// Rent and incentive breakdown derived from [`DaRentPolicyV1`].
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaRentQuote {
    /// Total rent charged for the blob (GiB × months × base rate).
    pub base_rent: XorQuantity,
    /// XOR routed to the protocol reserve fund.
    pub protocol_reserve: XorQuantity,
    /// XOR paid to storage providers for the base rent.
    pub provider_reward: XorQuantity,
    /// PDP success bonus (per evaluation cycle).
    pub pdp_bonus: XorQuantity,
    /// `PoTR` success bonus (per evaluation cycle).
    pub potr_bonus: XorQuantity,
    /// Credit per GiB of successful egress served to fetch clients.
    pub egress_credit_per_gib: XorQuantity,
}
impl Default for DaRentQuote {
    fn default() -> Self {
        Self {
            base_rent: XorQuantity::zero(),
            protocol_reserve: XorQuantity::zero(),
            provider_reward: XorQuantity::zero(),
            pdp_bonus: XorQuantity::zero(),
            potr_bonus: XorQuantity::zero(),
            egress_credit_per_gib: XorQuantity::zero(),
        }
    }
}
/// Ledger-oriented projection derived from a [`DaRentQuote`].
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DaRentLedgerProjection {
    /// Total rent owed for the retention period.
    pub rent_due: XorQuantity,
    /// Protocol-reserve allocation sourced from the rent.
    pub protocol_reserve_due: XorQuantity,
    /// Provider payout sourced from the rent (excludes bonuses).
    pub provider_reward_due: XorQuantity,
    /// PDP bonus pool earmarked per evaluation cycle.
    pub pdp_bonus_pool: XorQuantity,
    /// `PoTR` bonus pool earmarked per evaluation cycle.
    pub potr_bonus_pool: XorQuantity,
    /// Credit per GiB to reimburse fetch egress.
    pub egress_credit_per_gib: XorQuantity,
}
impl DaRentQuote {
    /// Project ledger-facing rent and incentive deltas.
    #[must_use]
    pub fn ledger_projection(&self) -> DaRentLedgerProjection {
        DaRentLedgerProjection {
            rent_due: self.base_rent.clone(),
            protocol_reserve_due: self.protocol_reserve.clone(),
            provider_reward_due: self.provider_reward.clone(),
            pdp_bonus_pool: self.pdp_bonus.clone(),
            potr_bonus_pool: self.potr_bonus.clone(),
            egress_credit_per_gib: self.egress_credit_per_gib.clone(),
        }
    }
}
/// Errors emitted while validating or quoting rent schedules.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum DaRentError {
    /// Unsupported schema version encountered.
    #[error("unsupported DA rent policy version: {found}")]
    UnsupportedVersion {
        /// Schema version reported by the payload.
        found: u8,
    },
    /// The configured basis-point ratio exceeds 100%.
    #[error("{field_label} exceeds BASIS_POINTS_PER_UNIT ({basis_points} bps)", field_label = field.label())]
    InvalidRatio {
        /// Ratio identifier.
        field: RentRatioField,
        /// Invalid basis-point value.
        basis_points: u16,
    },
    /// Base rate was zero.
    #[error("base_rate_per_gib_month must be non-zero")]
    ZeroRate,
    /// Zero-sized storage usage supplied.
    #[error("storage usage must be non-zero")]
    ZeroUsage,
    /// Zero-month retention supplied.
    #[error("retention duration must be non-zero")]
    ZeroDuration,
    /// Arithmetic overflow occurred.
    #[error("rent computation overflowed")]
    Overflow,
    /// A configured rate uses precision finer than the supported micro-XOR domain.
    #[error("DA rent rates support at most six fractional digits")]
    PrecisionTooFine,
}
/// Identifiers for basis-point ratios in [`DaRentPolicyV1`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RentRatioField {
    /// `protocol_reserve_bps`.
    ProtocolReserve,
    /// `pdp_bonus_bps`.
    PdpBonus,
    /// `potr_bonus_bps`.
    PotrBonus,
}
impl RentRatioField {
    const fn label(self) -> &'static str {
        match self {
            Self::ProtocolReserve => "protocol_reserve_bps",
            Self::PdpBonus => "pdp_bonus_bps",
            Self::PotrBonus => "potr_bonus_bps",
        }
    }
}
#[cfg(test)]
mod erasure_profile_tests {
    use super::*;
    use norito::json;
    #[test]
    fn skips_zero_row_parity_stripes_in_json() {
        let profile = ErasureProfile {
            row_parity_stripes: 0,
            ..Default::default()
        };
        let serialized = json::to_value(&profile)
            .and_then(|value| json::to_string(&value))
            .expect("serialize erasure profile");
        assert!(
            !serialized.contains("row_parity_stripes"),
            "zero stripes should be omitted: {serialized}"
        );
    }
    #[test]
    fn serializes_non_zero_row_parity_stripes_in_json() {
        let profile = ErasureProfile {
            row_parity_stripes: 2,
            ..Default::default()
        };
        let serialized = json::to_value(&profile)
            .and_then(|value| json::to_string(&value))
            .expect("serialize erasure profile");
        assert!(
            serialized.contains("\"row_parity_stripes\":2"),
            "non-zero stripes must be present: {serialized}"
        );
    }
}
#[cfg(test)]
mod rent_policy_tests {
    use super::*;
    use iroha_primitives::{bigint::BigInt, numeric::Numeric};
    const MAX_XOR: &str = "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047";
    const XOR_OVERFLOW: &str = "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042048";
    #[derive(Encode)]
    struct ForgedNumeric {
        #[codec(compact)]
        mantissa: BigInt,
        #[codec(compact)]
        scale: u32,
    }
    impl From<Numeric> for ForgedNumeric {
        fn from(value: Numeric) -> Self {
            Self {
                mantissa: value.mantissa().clone(),
                scale: value.scale(),
            }
        }
    }
    #[derive(Encode)]
    struct ForgedDaRentQuote {
        base_rent: ForgedNumeric,
        protocol_reserve: ForgedNumeric,
        provider_reward: ForgedNumeric,
        pdp_bonus: ForgedNumeric,
        potr_bonus: ForgedNumeric,
        egress_credit_per_gib: ForgedNumeric,
    }
    fn forged_quote(base_rent: ForgedNumeric) -> ForgedDaRentQuote {
        ForgedDaRentQuote {
            base_rent,
            protocol_reserve: Numeric::zero().into(),
            provider_reward: Numeric::zero().into(),
            pdp_bonus: Numeric::zero().into(),
            potr_bonus: Numeric::zero().into(),
            egress_credit_per_gib: Numeric::zero().into(),
        }
    }
    #[cfg(feature = "json")]
    fn quote_json(base_rent: &str) -> String {
        format!(
            r#"{{"base_rent":{base_rent},"protocol_reserve":"0","provider_reward":"0","pdp_bonus":"0","potr_bonus":"0","egress_credit_per_gib":"0"}}"#
        )
    }
    #[test]
    fn default_policy_quote_matches_expected_breakdown() {
        let policy = DaRentPolicyV1::default();
        let quote = policy.quote(10, 3).expect("rent quote");
        assert_eq!(quote.base_rent.to_string(), "7.5");
        assert_eq!(quote.protocol_reserve.to_string(), "1.5");
        assert_eq!(quote.provider_reward.to_string(), "6");
        assert_eq!(quote.pdp_bonus.to_string(), "0.375");
        assert_eq!(quote.potr_bonus.to_string(), "0.1875");
        assert_eq!(quote.egress_credit_per_gib.to_string(), "0.0015");
    }
    #[test]
    fn rent_quote_validates_inputs() {
        let policy = DaRentPolicyV1::default();
        assert!(matches!(policy.quote(0, 1), Err(DaRentError::ZeroUsage)));
        assert!(matches!(policy.quote(4, 0), Err(DaRentError::ZeroDuration)));
    }
    #[test]
    fn rent_policy_rejects_invalid_ratios() {
        let policy = DaRentPolicyV1 {
            protocol_reserve_bps: BASIS_POINTS_PER_UNIT + 1,
            ..DaRentPolicyV1::default()
        };
        assert!(matches!(
            policy.validate(),
            Err(DaRentError::InvalidRatio {
                field: RentRatioField::ProtocolReserve,
                ..
            })
        ));
    }
    #[test]
    fn rent_policy_rejects_finer_than_micro_rate_precision() {
        let policy = DaRentPolicyV1 {
            base_rate_per_gib_month: "0.0000001".parse().expect("canonical quantity"),
            ..DaRentPolicyV1::default()
        };
        assert_eq!(policy.validate(), Err(DaRentError::PrecisionTooFine));
    }
    #[test]
    fn rent_quote_rejects_512_bit_overflow() {
        let policy = DaRentPolicyV1 {
            base_rate_per_gib_month: MAX_XOR.parse().expect("maximum positive XOR mantissa"),
            ..DaRentPolicyV1::default()
        };
        assert_eq!(policy.quote(2, 1), Err(DaRentError::Overflow));
    }
    #[test]
    fn rent_split_rounds_toward_zero_at_policy_scale() {
        let policy = DaRentPolicyV1::from_components(
            "0.000001".parse().expect("micro-XOR base rate"),
            1,
            1,
            1,
            XorQuantity::zero(),
        );
        let quote = policy.quote(1, 1).expect("quote at rounding boundary");
        assert_eq!(quote.base_rent.to_string(), "0.000001");
        assert_eq!(quote.protocol_reserve, XorQuantity::zero());
        assert_eq!(quote.provider_reward.to_string(), "0.000001");
        assert_eq!(quote.pdp_bonus, XorQuantity::zero());
        assert_eq!(quote.potr_bonus, XorQuantity::zero());
    }
    #[test]
    fn rent_quote_norito_rejects_invalid_xor_payloads() {
        for invalid in [
            Numeric::new(-1_i32, 0).into(),
            Numeric::new(1_u32, 10).into(),
            ForgedNumeric {
                mantissa: BigInt::from(10_u32),
                scale: 1,
            },
            ForgedNumeric {
                mantissa: XOR_OVERFLOW.parse().expect("4096-bit bigint accepts 2^511"),
                scale: 0,
            },
        ] {
            let encoded = forged_quote(invalid).encode();
            assert!(
                DaRentQuote::decode(&mut encoded.as_slice()).is_err(),
                "signed, overprecise, noncanonical, or overflow payload must not cross the DA XOR boundary"
            );
        }
    }
    #[test]
    fn rent_quote_norito_accepts_scale_nine_maximum_and_wide_values() {
        for canonical in [
            "0.000000001",
            "340282366920938463463374607431768211456.000000001",
            MAX_XOR,
        ] {
            let quote = DaRentQuote {
                base_rent: canonical.parse().expect("valid XOR quantity"),
                ..DaRentQuote::default()
            };
            let encoded = quote.encode();
            let decoded = DaRentQuote::decode(&mut encoded.as_slice()).expect("decode quote");
            assert_eq!(decoded, quote, "canonical={canonical}");
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn rent_policy_json_uses_canonical_quantity_strings() {
        let json = norito::json::to_json(&DaRentPolicyV1::default()).expect("serialize policy");
        assert!(json.contains("\"base_rate_per_gib_month\":\"0.25\""));
        assert!(json.contains("\"egress_credit_per_gib\":\"0.0015\""));
    }
    #[cfg(feature = "json")]
    #[test]
    fn rent_quote_json_rejects_invalid_xor_representations() {
        for invalid in [
            "1",
            "true",
            "\"-1\"",
            "\"+1\"",
            "\"01\"",
            "\"1.0\"",
            "\" 1\"",
            "\"1e0\"",
            "\"0.0000000001\"",
            &format!("\"{XOR_OVERFLOW}\""),
        ] {
            let json = quote_json(invalid);
            assert!(
                norito::json::from_str::<DaRentQuote>(&json).is_err(),
                "invalid XOR JSON representation must be rejected: {invalid}"
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn rent_quote_json_roundtrips_submicro_wide_and_maximum_values() {
        for canonical in [
            "0.000000001",
            "340282366920938463463374607431768211456.000000001",
            MAX_XOR,
        ] {
            let json = quote_json(&format!("\"{canonical}\""));
            let decoded =
                norito::json::from_str::<DaRentQuote>(&json).expect("decode exact XOR quote");
            assert_eq!(decoded.base_rent.to_string(), canonical);
            let encoded = norito::json::to_json(&decoded).expect("encode exact XOR quote");
            assert!(encoded.contains(&format!("\"base_rent\":\"{canonical}\"")));
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn rent_policy_json_rejects_retired_micro_aliases() {
        let legacy = r#"{"version":1,"base_rate_per_gib_month_micro":250000,"protocol_reserve_bps":2000,"pdp_bonus_bps":500,"potr_bonus_bps":250,"egress_credit_per_gib_micro":1500}"#;
        assert!(
            norito::json::from_str::<DaRentPolicyV1>(legacy).is_err(),
            "retired implicit micro-XOR aliases must not decode"
        );
    }
    #[test]
    fn ledger_projection_reflects_quote_breakdown() {
        let policy = DaRentPolicyV1::default();
        let quote = policy.quote(5, 2).expect("rent quote");
        let projection = quote.ledger_projection();
        assert_eq!(projection.rent_due, quote.base_rent);
        assert_eq!(projection.protocol_reserve_due, quote.protocol_reserve);
        assert_eq!(projection.provider_reward_due, quote.provider_reward);
        assert_eq!(projection.pdp_bonus_pool, quote.pdp_bonus);
        assert_eq!(projection.potr_bonus_pool, quote.potr_bonus);
        assert_eq!(
            projection.egress_credit_per_gib,
            quote.egress_credit_per_gib
        );
    }
}
