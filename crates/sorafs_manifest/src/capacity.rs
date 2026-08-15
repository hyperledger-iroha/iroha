#![allow(unexpected_cfgs)]
//! Capacity marketplace schemas and validators for SoraFS (SF-2c).
//!
//! The Norito payloads defined here allow governance, Torii, and storage operators to exchange
//! deterministic capacity declarations, replication orders, and telemetry snapshots.
use crate::{
    chunker_registry,
    provider_advert::{CapabilityType, SignatureAlgorithm, StakePointer},
};
use ed25519_dalek::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use iroha_schema::IntoSchema;
use norito::{
    core::{DecodeFromSlice, decode_field_canonical},
    derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize},
};
use std::collections::BTreeSet;
use thiserror::Error;
/// Schema version for [`CapacityDeclarationV1`].
pub const CAPACITY_DECLARATION_VERSION_V1: u8 = 1;
/// Schema version for [`ReplicationOrderV1`].
pub const REPLICATION_ORDER_VERSION_V1: u8 = 1;
/// Schema version for [`SignedReplicationOrderV1`].
pub const SIGNED_REPLICATION_ORDER_VERSION_V1: u8 = 1;
/// Domain string included in canonical [`SignedReplicationOrderV1`] signing bytes.
pub const REPLICATION_ORDER_SIGNATURE_DOMAIN_V1: &str = "sorafs.replication_order.signature.v1";
/// Schema version for [`CapacityTelemetryV1`].
pub const CAPACITY_TELEMETRY_VERSION_V1: u8 = 1;
/// Schema version for [`CapacityDisputeV1`].
pub const CAPACITY_DISPUTE_VERSION_V1: u8 = 1;
/// Governance-authored statement of a provider's committed capacity.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct CapacityDeclarationV1 {
    /// Schema version (`CAPACITY_DECLARATION_VERSION_V1`).
    pub version: u8,
    /// Governance-controlled provider identifier (BLAKE3-256 digest).
    pub provider_id: [u8; 32],
    /// Stake pointer backing the commitment.
    pub stake: StakePointer,
    /// Total capacity committed across all chunker profiles (GiB).
    pub committed_capacity_gib: u64,
    /// Capacity slices for each supported chunker profile.
    pub chunker_commitments: Vec<ChunkerCommitmentV1>,
    /// Optional lane-specific capacity caps.
    #[norito(default)]
    pub lane_commitments: Vec<LaneCommitmentV1>,
    /// Optional pricing hints for governance tooling.
    #[norito(default)]
    pub pricing: Option<PricingScheduleV1>,
    /// Unix timestamp (seconds) when the declaration becomes active.
    pub valid_from: u64,
    /// Unix timestamp (seconds) when the declaration expires.
    pub valid_until: u64,
    /// Auxiliary metadata entries (key/value pairs).
    #[norito(default)]
    pub metadata: Vec<CapacityMetadataEntry>,
}
impl CapacityDeclarationV1 {
    /// Validates the declaration against registry policy.
    pub fn validate(&self) -> Result<(), CapacityDeclarationValidationError> {
        if self.version != CAPACITY_DECLARATION_VERSION_V1 {
            return Err(CapacityDeclarationValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(CapacityDeclarationValidationError::InvalidProviderId);
        }
        if !self.stake.is_positive() {
            return Err(CapacityDeclarationValidationError::StakeAmountZero);
        }
        if self.stake.pool_id.iter().all(|byte| *byte == 0) {
            return Err(CapacityDeclarationValidationError::InvalidStakePoolId);
        }
        if self.committed_capacity_gib == 0 {
            return Err(CapacityDeclarationValidationError::ZeroCommittedCapacity);
        }
        if self.chunker_commitments.is_empty() {
            return Err(CapacityDeclarationValidationError::MissingChunkerCommitments);
        }
        if self.chunker_commitments.len() > MAX_CAPACITY_CHUNKER_COMMITMENTS {
            return Err(
                CapacityDeclarationValidationError::TooManyChunkerCommitments {
                    found: self.chunker_commitments.len(),
                    maximum: MAX_CAPACITY_CHUNKER_COMMITMENTS,
                },
            );
        }
        if self.lane_commitments.len() > MAX_CAPACITY_LANE_COMMITMENTS {
            return Err(CapacityDeclarationValidationError::TooManyLaneCommitments {
                found: self.lane_commitments.len(),
                maximum: MAX_CAPACITY_LANE_COMMITMENTS,
            });
        }
        if self.metadata.len() > MAX_CAPACITY_DECLARATION_METADATA_ENTRIES {
            return Err(CapacityDeclarationValidationError::TooManyMetadataEntries {
                found: self.metadata.len(),
                maximum: MAX_CAPACITY_DECLARATION_METADATA_ENTRIES,
            });
        }
        if self.valid_until <= self.valid_from {
            return Err(CapacityDeclarationValidationError::InvalidValidityWindow);
        }
        let mut previous_profile: Option<&str> = None;
        let mut total_committed = 0u128;
        for (index, commitment) in self.chunker_commitments.iter().enumerate() {
            commitment.validate().map_err(|source| {
                CapacityDeclarationValidationError::ChunkerCommitmentInvalid { index, source }
            })?;
            if previous_profile.is_some_and(|previous| previous >= commitment.profile_id.as_str()) {
                return Err(CapacityDeclarationValidationError::NonCanonicalChunkerOrder { index });
            }
            previous_profile = Some(commitment.profile_id.as_str());
            total_committed = total_committed
                .checked_add(u128::from(commitment.committed_gib))
                .ok_or(CapacityDeclarationValidationError::CommittedCapacityOverflow)?;
        }
        if total_committed != u128::from(self.committed_capacity_gib) {
            return Err(
                CapacityDeclarationValidationError::CommittedCapacityMismatch {
                    committed_gib: total_committed,
                    declared_gib: self.committed_capacity_gib,
                },
            );
        }
        let mut previous_lane: Option<&str> = None;
        for (index, lane) in self.lane_commitments.iter().enumerate() {
            lane.validate(self.committed_capacity_gib)
                .map_err(
                    |source| CapacityDeclarationValidationError::LaneCommitmentInvalid {
                        index,
                        source,
                    },
                )?;
            if previous_lane.is_some_and(|previous| previous >= lane.lane_id.as_str()) {
                return Err(CapacityDeclarationValidationError::NonCanonicalLaneOrder { index });
            }
            previous_lane = Some(lane.lane_id.as_str());
        }
        if let Some(pricing) = &self.pricing {
            pricing
                .validate()
                .map_err(CapacityDeclarationValidationError::PricingInvalid)?;
        }
        let mut metadata_bytes = 0usize;
        let mut metadata_keys = BTreeSet::new();
        for (index, entry) in self.metadata.iter().enumerate() {
            entry.validate().map_err(|source| {
                CapacityDeclarationValidationError::MetadataInvalid { index, source }
            })?;
            metadata_bytes = metadata_bytes
                .checked_add(entry.key.len())
                .and_then(|total| total.checked_add(entry.value.len()))
                .ok_or(CapacityDeclarationValidationError::MetadataBytesExceeded {
                    found: usize::MAX,
                    maximum: MAX_CAPACITY_DECLARATION_METADATA_BYTES,
                })?;
            if metadata_bytes > MAX_CAPACITY_DECLARATION_METADATA_BYTES {
                return Err(CapacityDeclarationValidationError::MetadataBytesExceeded {
                    found: metadata_bytes,
                    maximum: MAX_CAPACITY_DECLARATION_METADATA_BYTES,
                });
            }
            if !metadata_keys.insert(entry.key.as_str()) {
                return Err(CapacityDeclarationValidationError::DuplicateMetadataKey {
                    key: entry.key.clone(),
                });
            }
        }
        Ok(())
    }
}
/// Per-profile capacity allocation.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct ChunkerCommitmentV1 {
    /// Canonical profile handle (`namespace.name@semver`).
    pub profile_id: String,
    /// Optional aliases accepted for negotiation.
    #[norito(default)]
    pub profile_aliases: Option<Vec<String>>,
    /// Capacity committed to the profile (GiB).
    pub committed_gib: u64,
    /// Capability references associated with the profile.
    #[norito(default)]
    pub capability_refs: Vec<CapabilityType>,
}
impl ChunkerCommitmentV1 {
    fn validate(
        &self,
    ) -> Result<&'static chunker_registry::ChunkerProfileDescriptor, ChunkerCommitmentError> {
        if self.committed_gib == 0 {
            return Err(ChunkerCommitmentError::ZeroCapacity);
        }
        if self.profile_id.len() > MAX_CAPACITY_PROFILE_HANDLE_BYTES {
            return Err(ChunkerCommitmentError::ProfileHandleTooLong {
                found: self.profile_id.len(),
                maximum: MAX_CAPACITY_PROFILE_HANDLE_BYTES,
            });
        }
        let descriptor = chunker_registry::lookup_by_handle(&self.profile_id).ok_or(
            ChunkerCommitmentError::UnknownProfileHandle {
                handle: self.profile_id.clone(),
            },
        )?;
        let canonical = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        if self.profile_id != canonical {
            return Err(ChunkerCommitmentError::NonCanonicalHandle {
                provided: self.profile_id.clone(),
                canonical,
            });
        }
        if let Some(aliases) = &self.profile_aliases {
            if aliases.is_empty() || aliases.len() > MAX_CAPACITY_PROFILE_ALIASES {
                return Err(ChunkerCommitmentError::InvalidProfileAliasCount {
                    found: aliases.len(),
                    maximum: MAX_CAPACITY_PROFILE_ALIASES,
                });
            }
            for alias in aliases {
                if alias.len() > MAX_CAPACITY_PROFILE_HANDLE_BYTES {
                    return Err(ChunkerCommitmentError::ProfileAliasTooLong {
                        found: alias.len(),
                        maximum: MAX_CAPACITY_PROFILE_HANDLE_BYTES,
                    });
                }
            }
            validate_aliases(aliases, descriptor).map_err(ChunkerCommitmentError::from)?;
            if aliases.len() != descriptor.aliases.len()
                || aliases
                    .iter()
                    .zip(descriptor.aliases)
                    .any(|(provided, expected)| provided.as_str() != *expected)
            {
                return Err(ChunkerCommitmentError::NonCanonicalProfileAliases);
            }
        }
        if self.capability_refs.len() > MAX_CAPACITY_CAPABILITY_REFS {
            return Err(ChunkerCommitmentError::TooManyCapabilities {
                found: self.capability_refs.len(),
                maximum: MAX_CAPACITY_CAPABILITY_REFS,
            });
        }
        let mut previous_capability = None;
        for (index, capability) in self.capability_refs.iter().enumerate() {
            let key = *capability as u16;
            if previous_capability.is_some_and(|previous| previous >= key) {
                return Err(ChunkerCommitmentError::NonCanonicalCapabilityOrder { index });
            }
            previous_capability = Some(key);
        }
        Ok(descriptor)
    }
}
/// Upper-bound commitment for a capacity lane.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct LaneCommitmentV1 {
    /// Lane identifier (e.g., `hot`, `bridge`, `regulatory`).
    pub lane_id: String,
    /// Maximum capacity allocatable to the lane (GiB).
    pub max_gib: u64,
}
impl LaneCommitmentV1 {
    fn validate(&self, total_capacity: u64) -> Result<(), LaneCommitmentError> {
        if self.lane_id.trim().is_empty() {
            return Err(LaneCommitmentError::EmptyLaneId);
        }
        if self.lane_id != self.lane_id.trim() || self.lane_id.len() > MAX_CAPACITY_LANE_ID_BYTES {
            return Err(LaneCommitmentError::NonCanonicalLaneId {
                found: self.lane_id.len(),
                maximum: MAX_CAPACITY_LANE_ID_BYTES,
            });
        }
        if !self
            .lane_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(LaneCommitmentError::InvalidLaneId {
                lane_id: self.lane_id.clone(),
            });
        }
        if self.max_gib == 0 {
            return Err(LaneCommitmentError::ZeroCapacity);
        }
        if self.max_gib > total_capacity {
            return Err(LaneCommitmentError::CapacityExceedsDeclaration {
                lane_gib: self.max_gib,
                declared_gib: total_capacity,
            });
        }
        Ok(())
    }
}
/// Pricing hints attached to a capacity declaration.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct PricingScheduleV1 {
    /// Three-to-six character currency code (e.g., `xor`).
    pub currency: String,
    /// Price per GiB·hour in milli-units of the currency.
    pub rate_per_gib_hour_milliu: u64,
    /// Optional minimum commitment duration (hours).
    #[norito(default)]
    pub min_commitment_hours: Option<u32>,
    /// Optional operator notes.
    #[norito(default)]
    pub notes: Option<String>,
}
impl PricingScheduleV1 {
    fn validate(&self) -> Result<(), PricingScheduleError> {
        let trimmed = self.currency.trim();
        if trimmed.is_empty() {
            return Err(PricingScheduleError::EmptyCurrency);
        }
        if trimmed.len() < 3 || trimmed.len() > 6 {
            return Err(PricingScheduleError::InvalidCurrencyLength);
        }
        if !trimmed
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return Err(PricingScheduleError::InvalidCurrencyChars);
        }
        if trimmed != self.currency {
            return Err(PricingScheduleError::NonCanonicalCurrency);
        }
        if self.rate_per_gib_hour_milliu == 0 {
            return Err(PricingScheduleError::ZeroRate);
        }
        if self.min_commitment_hours == Some(0) {
            return Err(PricingScheduleError::InvalidMinCommitment);
        }
        if let Some(notes) = &self.notes
            && (notes.trim().is_empty()
                || notes != notes.trim()
                || notes.len() > MAX_CAPACITY_PRICING_NOTES_BYTES
                || notes.chars().any(char::is_control))
        {
            return Err(PricingScheduleError::InvalidNotes);
        }
        Ok(())
    }
}
impl<'a> DecodeFromSlice<'a> for PricingScheduleV1 {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        decode_field_canonical::<PricingScheduleV1>(bytes)
    }
}
/// Metadata entry attached to capacity artefacts.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct CapacityMetadataEntry {
    /// Metadata key (`[a-z0-9_.-]+`).
    pub key: String,
    /// Metadata value (non-empty UTF-8).
    pub value: String,
}
impl CapacityMetadataEntry {
    pub fn validate(&self) -> Result<(), MetadataError> {
        let key_trimmed = self.key.trim();
        if key_trimmed.is_empty() {
            return Err(MetadataError::EmptyKey);
        }
        if !key_trimmed.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '_' || c == '-'
        }) {
            return Err(MetadataError::InvalidKey {
                key: self.key.clone(),
            });
        }
        if key_trimmed != self.key {
            return Err(MetadataError::NonCanonicalKey);
        }
        if self.key.len() > MAX_CAPACITY_METADATA_KEY_BYTES {
            return Err(MetadataError::KeyTooLong {
                found: self.key.len(),
                maximum: MAX_CAPACITY_METADATA_KEY_BYTES,
            });
        }
        if self.value.trim().is_empty() {
            return Err(MetadataError::EmptyValue);
        }
        if self.value != self.value.trim()
            || self.value.len() > MAX_CAPACITY_METADATA_VALUE_BYTES
            || self.value.chars().any(char::is_control)
        {
            return Err(MetadataError::InvalidValue {
                found: self.value.len(),
                maximum: MAX_CAPACITY_METADATA_VALUE_BYTES,
            });
        }
        Ok(())
    }
}
/// Maximum key length for capacity/order metadata.
pub const MAX_CAPACITY_METADATA_KEY_BYTES: usize = 128;
/// Maximum value length for capacity/order metadata.
pub const MAX_CAPACITY_METADATA_VALUE_BYTES: usize = 4096;
/// Exact first-release manifest CID length in a replication order.
pub const MAX_REPLICATION_ORDER_MANIFEST_CID_BYTES: usize = crate::MAX_MANIFEST_ROOT_CID_BYTES;
/// Maximum provider assignments in one replication order.
pub const MAX_REPLICATION_ORDER_ASSIGNMENTS: usize = 1024;
/// Maximum metadata entries in one replication order.
pub const MAX_REPLICATION_ORDER_METADATA_ENTRIES: usize = 64;
/// Maximum aggregate metadata bytes in one replication order.
pub const MAX_REPLICATION_ORDER_METADATA_BYTES: usize = 64 * 1024;
/// Maximum canonical chunker-handle length.
pub const MAX_REPLICATION_ORDER_PROFILE_BYTES: usize = 128;
/// Maximum lane-hint length.
pub const MAX_REPLICATION_ORDER_LANE_BYTES: usize = 64;
/// Maximum chunker profiles in one capacity declaration.
pub const MAX_CAPACITY_CHUNKER_COMMITMENTS: usize = 16;
/// Maximum lane caps in one capacity declaration.
pub const MAX_CAPACITY_LANE_COMMITMENTS: usize = 64;
/// Maximum metadata entries in one capacity declaration.
pub const MAX_CAPACITY_DECLARATION_METADATA_ENTRIES: usize = 64;
/// Maximum aggregate metadata bytes in one capacity declaration.
pub const MAX_CAPACITY_DECLARATION_METADATA_BYTES: usize = 64 * 1024;
/// Maximum aliases in one chunker commitment.
pub const MAX_CAPACITY_PROFILE_ALIASES: usize = 16;
/// Maximum capability references in one chunker commitment.
pub const MAX_CAPACITY_CAPABILITY_REFS: usize = 32;
/// Maximum canonical profile-handle length.
pub const MAX_CAPACITY_PROFILE_HANDLE_BYTES: usize = 128;
/// Maximum lane identifier length.
pub const MAX_CAPACITY_LANE_ID_BYTES: usize = 64;
/// Maximum pricing-note length.
pub const MAX_CAPACITY_PRICING_NOTES_BYTES: usize = 1024;
/// Replication order issued by governance.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ReplicationOrderV1 {
    /// Schema version (`REPLICATION_ORDER_VERSION_V1`).
    pub version: u8,
    /// Governance-controlled order identifier (BLAKE3-256 digest).
    pub order_id: [u8; 32],
    /// Manifest CID targeted by the order.
    pub manifest_cid: Vec<u8>,
    /// Canonical manifest digest (BLAKE3-256).
    pub manifest_digest: [u8; 32],
    /// Canonical chunker handle required for the manifest.
    pub chunking_profile: String,
    /// Desired redundancy level (number of replicas).
    pub target_replicas: u16,
    /// Provider assignments that should store the manifest.
    pub assignments: Vec<ReplicationAssignmentV1>,
    /// Unix timestamp (seconds) when the order was issued.
    pub issued_at: u64,
    /// Unix timestamp (seconds) when ingestion must be complete.
    pub deadline_at: u64,
    /// Service-level agreement expectations.
    pub sla: ReplicationOrderSlaV1,
    /// Additional metadata (optional).
    #[norito(default)]
    pub metadata: Vec<CapacityMetadataEntry>,
}
impl ReplicationOrderV1 {
    /// Validates the replication order structure.
    pub fn validate(&self) -> Result<(), ReplicationOrderValidationError> {
        if self.version != REPLICATION_ORDER_VERSION_V1 {
            return Err(ReplicationOrderValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.order_id.iter().all(|&byte| byte == 0) {
            return Err(ReplicationOrderValidationError::InvalidOrderId);
        }
        crate::validate_manifest_root_cid(
            &self.manifest_cid,
            crate::chunker_registry::MANIFEST_DAG_CODEC,
            crate::BLAKE3_256_MULTIHASH_CODE,
        )
        .map_err(|error| match error {
            crate::ManifestValidationError::InvalidRootCidLength { found, maximum } => {
                ReplicationOrderValidationError::InvalidManifestCidLength { found, maximum }
            }
            crate::ManifestValidationError::InertRootCid
            | crate::ManifestValidationError::InertRootCidDigest => {
                ReplicationOrderValidationError::InertManifestCid
            }
            error => ReplicationOrderValidationError::MalformedManifestCid {
                reason: error.to_string(),
            },
        })?;
        if self.manifest_digest.iter().all(|&byte| byte == 0) {
            return Err(ReplicationOrderValidationError::InvalidManifestDigest);
        }
        if self.chunking_profile.len() > MAX_REPLICATION_ORDER_PROFILE_BYTES {
            return Err(ReplicationOrderValidationError::ProfileHandleTooLong {
                found: self.chunking_profile.len(),
                maximum: MAX_REPLICATION_ORDER_PROFILE_BYTES,
            });
        }
        let descriptor = chunker_registry::lookup_by_handle(&self.chunking_profile).ok_or(
            ReplicationOrderValidationError::UnknownChunkerHandle {
                handle: self.chunking_profile.clone(),
            },
        )?;
        let canonical = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        if self.chunking_profile != canonical {
            return Err(ReplicationOrderValidationError::NonCanonicalProfileHandle {
                provided: self.chunking_profile.clone(),
                canonical,
            });
        }
        if self.target_replicas == 0 {
            return Err(ReplicationOrderValidationError::ZeroReplicaTarget);
        }
        if self.assignments.is_empty() {
            return Err(ReplicationOrderValidationError::MissingAssignments);
        }
        if self.assignments.len() > MAX_REPLICATION_ORDER_ASSIGNMENTS {
            return Err(ReplicationOrderValidationError::TooManyAssignments {
                found: self.assignments.len(),
                maximum: MAX_REPLICATION_ORDER_ASSIGNMENTS,
            });
        }
        if usize::from(self.target_replicas) > self.assignments.len() {
            return Err(
                ReplicationOrderValidationError::ReplicaTargetExceedsAssignments {
                    target: self.target_replicas,
                    assignments: self.assignments.len() as u16,
                },
            );
        }
        if self.deadline_at <= self.issued_at {
            return Err(ReplicationOrderValidationError::InvalidDeadline);
        }
        let mut previous_provider = None;
        for (index, assignment) in self.assignments.iter().enumerate() {
            assignment.validate().map_err(|source| {
                ReplicationOrderValidationError::AssignmentInvalid { index, source }
            })?;
            if previous_provider.is_some_and(|previous| previous >= assignment.provider_id) {
                return Err(ReplicationOrderValidationError::NonCanonicalProviderOrder { index });
            }
            previous_provider = Some(assignment.provider_id);
        }
        self.sla
            .validate()
            .map_err(ReplicationOrderValidationError::SlaInvalid)?;
        if u64::from(self.sla.ingest_deadline_secs) > self.deadline_at - self.issued_at {
            return Err(ReplicationOrderValidationError::SlaExceedsOrderWindow);
        }
        if self.metadata.len() > MAX_REPLICATION_ORDER_METADATA_ENTRIES {
            return Err(ReplicationOrderValidationError::TooManyMetadataEntries {
                found: self.metadata.len(),
                maximum: MAX_REPLICATION_ORDER_METADATA_ENTRIES,
            });
        }
        let mut metadata_bytes = 0usize;
        let mut metadata_keys = BTreeSet::new();
        for (index, entry) in self.metadata.iter().enumerate() {
            entry.validate().map_err(|source| {
                ReplicationOrderValidationError::MetadataInvalid { index, source }
            })?;
            metadata_bytes = metadata_bytes
                .checked_add(entry.key.len())
                .and_then(|total| total.checked_add(entry.value.len()))
                .ok_or(ReplicationOrderValidationError::MetadataBytesExceeded {
                    found: usize::MAX,
                    maximum: MAX_REPLICATION_ORDER_METADATA_BYTES,
                })?;
            if metadata_bytes > MAX_REPLICATION_ORDER_METADATA_BYTES {
                return Err(ReplicationOrderValidationError::MetadataBytesExceeded {
                    found: metadata_bytes,
                    maximum: MAX_REPLICATION_ORDER_METADATA_BYTES,
                });
            }
            if !metadata_keys.insert(entry.key.as_str()) {
                return Err(ReplicationOrderValidationError::DuplicateMetadataKey {
                    key: entry.key.clone(),
                });
            }
        }
        Ok(())
    }
}
/// Signature attached to a governance-issued replication order.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ReplicationOrderSignatureV1 {
    /// Signature algorithm.
    pub algorithm: SignatureAlgorithm,
    /// Governance issuer public key.
    pub public_key: Vec<u8>,
    /// Raw signature bytes.
    pub signature: Vec<u8>,
}
impl ReplicationOrderSignatureV1 {
    fn validate(&self) -> Result<(), SignedReplicationOrderValidationError> {
        if self.public_key.is_empty()
            || crate::inert_bytes(&self.public_key)
            || self.signature.is_empty()
            || crate::inert_bytes(&self.signature)
        {
            return Err(SignedReplicationOrderValidationError::InvalidSignature);
        }
        Ok(())
    }
}
/// Signed governance envelope for a replication order.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct SignedReplicationOrderV1 {
    /// Schema version (`SIGNED_REPLICATION_ORDER_VERSION_V1`).
    pub version: u8,
    /// Replication order being authorized.
    pub order: ReplicationOrderV1,
    /// Governance signature over the canonical order signing payload.
    pub signature: ReplicationOrderSignatureV1,
}
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct ReplicationOrderSigningPayloadV1 {
    domain: String,
    version: u8,
    order: ReplicationOrderV1,
}
impl From<&SignedReplicationOrderV1> for ReplicationOrderSigningPayloadV1 {
    fn from(envelope: &SignedReplicationOrderV1) -> Self {
        Self {
            domain: REPLICATION_ORDER_SIGNATURE_DOMAIN_V1.to_owned(),
            version: envelope.version,
            order: envelope.order.clone(),
        }
    }
}
impl SignedReplicationOrderV1 {
    /// Validates the signed replication-order envelope structure.
    pub fn validate(&self) -> Result<(), SignedReplicationOrderValidationError> {
        if self.version != SIGNED_REPLICATION_ORDER_VERSION_V1 {
            return Err(SignedReplicationOrderValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        self.order
            .validate()
            .map_err(SignedReplicationOrderValidationError::Order)?;
        self.signature.validate()?;
        Ok(())
    }
    /// Returns canonical Norito bytes signed by the governance issuer.
    ///
    /// The payload excludes `signature` so signers and verifiers use stable bytes
    /// before and after the signature is attached.
    pub fn signature_payload_bytes(&self) -> Result<Vec<u8>, norito::core::Error> {
        norito::to_bytes(&ReplicationOrderSigningPayloadV1::from(self))
    }
    /// Verifies an Ed25519 signature over the canonical order signing payload.
    pub fn verify_signature(&self) -> Result<(), ReplicationOrderSignatureVerificationError> {
        match self.signature.algorithm {
            SignatureAlgorithm::Ed25519 => {}
            other => {
                return Err(
                    ReplicationOrderSignatureVerificationError::UnsupportedAlgorithm(other),
                );
            }
        }
        if self.signature.public_key.len() != PUBLIC_KEY_LENGTH {
            return Err(
                ReplicationOrderSignatureVerificationError::InvalidPublicKeyLength {
                    length: self.signature.public_key.len(),
                },
            );
        }
        if self.signature.signature.len() != SIGNATURE_LENGTH {
            return Err(
                ReplicationOrderSignatureVerificationError::InvalidSignatureLength {
                    length: self.signature.signature.len(),
                },
            );
        }
        let mut public_key = [0u8; PUBLIC_KEY_LENGTH];
        public_key.copy_from_slice(&self.signature.public_key);
        let verifying_key =
            crate::checked_ed25519_verifying_key_from_bytes(&public_key).map_err(|err| {
                ReplicationOrderSignatureVerificationError::InvalidPublicKey { reason: err }
            })?;
        let mut signature = [0u8; SIGNATURE_LENGTH];
        signature.copy_from_slice(&self.signature.signature);
        let signature =
            crate::checked_ed25519_signature_from_bytes(&signature).map_err(|reason| {
                ReplicationOrderSignatureVerificationError::Verification { reason }
            })?;
        let payload_bytes = self.signature_payload_bytes().map_err(|err| {
            ReplicationOrderSignatureVerificationError::PayloadEncoding {
                reason: err.to_string(),
            }
        })?;
        verifying_key
            .verify_strict(&payload_bytes, &signature)
            .map_err(
                |err| ReplicationOrderSignatureVerificationError::Verification {
                    reason: err.to_string(),
                },
            )
    }
}
/// Assignment binding a provider to store a manifest slice.
#[derive(
    Debug,
    Clone,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    IntoSchema,
)]
pub struct ReplicationAssignmentV1 {
    /// Provider identifier as authorised by governance.
    pub provider_id: [u8; 32],
    /// Capacity slice to allocate (GiB).
    pub slice_gib: u64,
    /// Optional lane hint to satisfy policy constraints.
    #[norito(default)]
    pub lane: Option<String>,
}
impl ReplicationAssignmentV1 {
    fn validate(&self) -> Result<(), AssignmentError> {
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(AssignmentError::InvalidProviderId);
        }
        if self.slice_gib == 0 {
            return Err(AssignmentError::ZeroSlice);
        }
        if let Some(lane) = &self.lane
            && (lane.is_empty()
                || lane.len() > MAX_REPLICATION_ORDER_LANE_BYTES
                || lane != lane.trim()
                || !lane.bytes().all(|byte| {
                    byte.is_ascii_lowercase() || byte.is_ascii_digit() || b".-_".contains(&byte)
                }))
        {
            return Err(AssignmentError::InvalidLane);
        }
        Ok(())
    }
}
/// SLA constraints attached to a replication order.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct ReplicationOrderSlaV1 {
    /// Ingestion deadline in seconds from order issuance.
    pub ingest_deadline_secs: u32,
    /// Minimum availability percentage (scaled by 1000).
    pub min_availability_percent_milli: u32,
    /// Minimum proof-of-replication success percentage (scaled by 1000).
    pub min_por_success_percent_milli: u32,
}
impl ReplicationOrderSlaV1 {
    fn validate(&self) -> Result<(), SlaError> {
        if self.ingest_deadline_secs == 0 {
            return Err(SlaError::InvalidIngestDeadline);
        }
        if self.min_availability_percent_milli == 0 || self.min_availability_percent_milli > 100_000
        {
            return Err(SlaError::AvailabilityOutOfRange {
                value: self.min_availability_percent_milli,
            });
        }
        if self.min_por_success_percent_milli == 0 || self.min_por_success_percent_milli > 100_000 {
            return Err(SlaError::PorOutOfRange {
                value: self.min_por_success_percent_milli,
            });
        }
        Ok(())
    }
}
/// Telemetry snapshot emitted for fee distribution.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct CapacityTelemetryV1 {
    /// Schema version (`CAPACITY_TELEMETRY_VERSION_V1`).
    pub version: u8,
    /// Provider identifier the telemetry applies to.
    pub provider_id: [u8; 32],
    /// Epoch start timestamp (seconds since Unix epoch).
    pub epoch_start: u64,
    /// Epoch end timestamp (seconds since Unix epoch).
    pub epoch_end: u64,
    /// Declared capacity during the epoch (GiB).
    pub declared_capacity_gib: u64,
    /// Average utilised capacity during the epoch (GiB).
    pub utilised_capacity_gib: u64,
    /// Successful replication count during the epoch.
    pub successful_replications: u32,
    /// Failed replication count during the epoch.
    pub failed_replications: u32,
    /// Uptime percentage (scaled by 1000, e.g., 99500 = 99.5%).
    pub uptime_percent_milli: u32,
    /// Proof-of-replication success percentage (scaled by 1000).
    pub por_success_percent_milli: u32,
    /// Optional telemetry notes.
    #[norito(default)]
    pub notes: Option<String>,
}
impl CapacityTelemetryV1 {
    /// Validates the telemetry payload.
    pub fn validate(&self) -> Result<(), CapacityTelemetryValidationError> {
        if self.version != CAPACITY_TELEMETRY_VERSION_V1 {
            return Err(CapacityTelemetryValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(CapacityTelemetryValidationError::InvalidProviderId);
        }
        if self.epoch_end <= self.epoch_start {
            return Err(CapacityTelemetryValidationError::InvalidEpochRange);
        }
        if self.utilised_capacity_gib > self.declared_capacity_gib {
            return Err(
                CapacityTelemetryValidationError::UtilisationExceedsDeclaration {
                    utilised_gib: self.utilised_capacity_gib,
                    declared_gib: self.declared_capacity_gib,
                },
            );
        }
        if self.uptime_percent_milli > 100_000 {
            return Err(CapacityTelemetryValidationError::PercentOutOfRange {
                field: "uptime_percent_milli",
                value: self.uptime_percent_milli,
            });
        }
        if self.por_success_percent_milli > 100_000 {
            return Err(CapacityTelemetryValidationError::PercentOutOfRange {
                field: "por_success_percent_milli",
                value: self.por_success_percent_milli,
            });
        }
        if let Some(notes) = &self.notes
            && notes.trim().is_empty()
        {
            return Err(CapacityTelemetryValidationError::InvalidNotes);
        }
        Ok(())
    }
}
/// Maximum length permitted for dispute descriptions.
const MAX_DISPUTE_DESCRIPTION_LEN: usize = 2_048;
/// Maximum length permitted for dispute remediation notes.
const MAX_DISPUTE_REMEDY_LEN: usize = 512;
/// Maximum length permitted for evidence URIs.
const MAX_DISPUTE_EVIDENCE_URI_LEN: usize = 512;
/// Maximum length permitted for evidence media types.
const MAX_DISPUTE_EVIDENCE_MEDIA_LEN: usize = 128;
/// Enumerates dispute categories recognised by governance.
#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum CapacityDisputeKind {
    /// Provider failed to meet replication order requirements.
    ReplicationShortfall = 1,
    /// Provider uptime dipped below the configured SLA.
    UptimeBreach = 2,
    /// Provider failed proof-of-retrievability sampling.
    ProofFailure = 3,
    /// Disagreement over fee accrual or billing statements.
    FeeDispute = 4,
    /// Custom dispute reason (see description field for details).
    Other = 255,
}
/// Evidence bundle describing supporting material for a dispute.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct CapacityDisputeEvidenceV1 {
    /// Deterministic digest (BLAKE3-256) of the evidence bundle.
    pub evidence_digest: [u8; 32],
    /// Optional media type describing the evidence payload (e.g. `application/zip`).
    #[norito(default)]
    pub media_type: Option<String>,
    /// Optional URI pointing to the evidence bundle.
    #[norito(default)]
    pub uri: Option<String>,
    /// Optional size of the evidence bundle in bytes.
    #[norito(default)]
    pub size_bytes: Option<u64>,
}
impl CapacityDisputeEvidenceV1 {
    fn validate(&self) -> Result<(), CapacityDisputeEvidenceError> {
        if self.evidence_digest.iter().all(|&byte| byte == 0) {
            return Err(CapacityDisputeEvidenceError::DigestZero);
        }
        if let Some(media) = &self.media_type {
            let trimmed = media.trim();
            if trimmed.is_empty() {
                return Err(CapacityDisputeEvidenceError::MediaTypeLength {
                    len: media.len(),
                    max: MAX_DISPUTE_EVIDENCE_MEDIA_LEN,
                });
            }
            if trimmed.len() > MAX_DISPUTE_EVIDENCE_MEDIA_LEN {
                return Err(CapacityDisputeEvidenceError::MediaTypeLength {
                    len: trimmed.len(),
                    max: MAX_DISPUTE_EVIDENCE_MEDIA_LEN,
                });
            }
        }
        if let Some(uri) = &self.uri {
            let trimmed = uri.trim();
            if trimmed.is_empty() {
                return Err(CapacityDisputeEvidenceError::UriLength {
                    len: uri.len(),
                    max: MAX_DISPUTE_EVIDENCE_URI_LEN,
                });
            }
            if trimmed.len() > MAX_DISPUTE_EVIDENCE_URI_LEN {
                return Err(CapacityDisputeEvidenceError::UriLength {
                    len: trimmed.len(),
                    max: MAX_DISPUTE_EVIDENCE_URI_LEN,
                });
            }
        }
        if let Some(size) = self.size_bytes
            && size == 0
        {
            return Err(CapacityDisputeEvidenceError::SizeNonPositive);
        }
        Ok(())
    }
}
/// Governance-authored dispute payload targeting a storage provider.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct CapacityDisputeV1 {
    /// Schema version (`CAPACITY_DISPUTE_VERSION_V1`).
    pub version: u8,
    /// Identifier of the provider targeted by the dispute.
    pub provider_id: [u8; 32],
    /// Identifier for the complainant (BLAKE3-256 digest of the account).
    pub complainant_id: [u8; 32],
    /// Optional replication order identifier associated with the dispute.
    #[norito(default)]
    pub replication_order_id: Option<[u8; 32]>,
    /// Dispute category.
    pub kind: CapacityDisputeKind,
    /// Evidence bundle metadata.
    pub evidence: CapacityDisputeEvidenceV1,
    /// Epoch (seconds since Unix epoch) when the dispute was submitted.
    pub submitted_epoch: u64,
    /// Human-readable description summarising the dispute.
    pub description: String,
    /// Optional requested remedy proposed by the complainant.
    #[norito(default)]
    pub requested_remedy: Option<String>,
}
impl CapacityDisputeV1 {
    /// Validates the dispute payload.
    pub fn validate(&self) -> Result<(), CapacityDisputeValidationError> {
        if self.version != CAPACITY_DISPUTE_VERSION_V1 {
            return Err(CapacityDisputeValidationError::UnsupportedVersion {
                found: self.version,
            });
        }
        if self.provider_id.iter().all(|&byte| byte == 0) {
            return Err(CapacityDisputeValidationError::InvalidProviderId);
        }
        if self.complainant_id.iter().all(|&byte| byte == 0) {
            return Err(CapacityDisputeValidationError::InvalidComplainantId);
        }
        if let Some(order_id) = self.replication_order_id
            && order_id.iter().all(|&byte| byte == 0)
        {
            return Err(CapacityDisputeValidationError::InvalidReplicationOrderId);
        }
        let description_trimmed = self.description.trim();
        if description_trimmed.is_empty() {
            return Err(CapacityDisputeValidationError::InvalidDescription {
                len: self.description.len(),
                max: MAX_DISPUTE_DESCRIPTION_LEN,
            });
        }
        if description_trimmed.len() > MAX_DISPUTE_DESCRIPTION_LEN {
            return Err(CapacityDisputeValidationError::InvalidDescription {
                len: description_trimmed.len(),
                max: MAX_DISPUTE_DESCRIPTION_LEN,
            });
        }
        if let Some(remedy) = &self.requested_remedy {
            let trimmed = remedy.trim();
            if trimmed.is_empty() {
                return Err(CapacityDisputeValidationError::InvalidRemedy {
                    len: remedy.len(),
                    max: MAX_DISPUTE_REMEDY_LEN,
                });
            }
            if trimmed.len() > MAX_DISPUTE_REMEDY_LEN {
                return Err(CapacityDisputeValidationError::InvalidRemedy {
                    len: trimmed.len(),
                    max: MAX_DISPUTE_REMEDY_LEN,
                });
            }
        }
        self.evidence
            .validate()
            .map_err(CapacityDisputeValidationError::Evidence)?;
        Ok(())
    }
}
/// Errors raised when validating capacity declarations.
#[derive(Debug, Error)]
pub enum CapacityDeclarationValidationError {
    #[error("unsupported capacity declaration version: {found}")]
    UnsupportedVersion { found: u8 },
    #[error("provider identifier must be non-zero")]
    InvalidProviderId,
    #[error("stake amount must be positive")]
    StakeAmountZero,
    #[error("stake pool identifier must be non-zero")]
    InvalidStakePoolId,
    #[error("total committed capacity must be positive")]
    ZeroCommittedCapacity,
    #[error("at least one chunker commitment is required")]
    MissingChunkerCommitments,
    #[error("capacity declaration has {found} chunker commitments; maximum is {maximum}")]
    TooManyChunkerCommitments { found: usize, maximum: usize },
    #[error("chunker commitments must be distinct and strictly ordered (index {index})")]
    NonCanonicalChunkerOrder { index: usize },
    #[error(
        "chunker commitment total {committed_gib} GiB does not equal declared {declared_gib} GiB"
    )]
    CommittedCapacityMismatch {
        committed_gib: u128,
        declared_gib: u64,
    },
    #[error("chunker commitment {index} invalid: {source}")]
    ChunkerCommitmentInvalid {
        index: usize,
        source: ChunkerCommitmentError,
    },
    #[error("lane commitment {index} invalid: {source}")]
    LaneCommitmentInvalid {
        index: usize,
        source: LaneCommitmentError,
    },
    #[error("capacity declaration has {found} lane commitments; maximum is {maximum}")]
    TooManyLaneCommitments { found: usize, maximum: usize },
    #[error("lane commitments must be distinct and strictly ordered (index {index})")]
    NonCanonicalLaneOrder { index: usize },
    #[error("pricing schedule invalid: {0}")]
    PricingInvalid(PricingScheduleError),
    #[error("valid_until must be greater than valid_from")]
    InvalidValidityWindow,
    #[error("metadata entry {index} invalid: {source}")]
    MetadataInvalid { index: usize, source: MetadataError },
    #[error("capacity declaration has {found} metadata entries; maximum is {maximum}")]
    TooManyMetadataEntries { found: usize, maximum: usize },
    #[error("capacity declaration metadata contains {found} bytes; maximum is {maximum}")]
    MetadataBytesExceeded { found: usize, maximum: usize },
    #[error("duplicate capacity declaration metadata key `{key}`")]
    DuplicateMetadataKey { key: String },
    #[error("committed capacity overflowed 128-bit accumulator")]
    CommittedCapacityOverflow,
}
/// Errors raised when validating chunker commitments.
#[derive(Debug, Error)]
pub enum ChunkerCommitmentError {
    #[error("chunker profile handle has {found} bytes; maximum is {maximum}")]
    ProfileHandleTooLong { found: usize, maximum: usize },
    #[error("unknown chunker profile handle: {handle}")]
    UnknownProfileHandle { handle: String },
    #[error("chunker handle must be canonical (provided {provided}, canonical {canonical})")]
    NonCanonicalHandle { provided: String, canonical: String },
    #[error("chunker commitment must allocate a positive number of GiB")]
    ZeroCapacity,
    #[error("chunker aliases missing canonical entry {canonical}")]
    MissingCanonicalAlias { canonical: String },
    #[error("chunker alias {alias} not recognised by registry")]
    UnknownProfileAlias { alias: String },
    #[error("chunker alias list must be non-empty and trimmed")]
    InvalidProfileAliases,
    #[error("chunker alias list has {found} entries; maximum is {maximum}")]
    InvalidProfileAliasCount { found: usize, maximum: usize },
    #[error("chunker alias has {found} bytes; maximum is {maximum}")]
    ProfileAliasTooLong { found: usize, maximum: usize },
    #[error("chunker aliases must exactly match the canonical registry order")]
    NonCanonicalProfileAliases,
    #[error("chunker commitment has {found} capabilities; maximum is {maximum}")]
    TooManyCapabilities { found: usize, maximum: usize },
    #[error("capability references must be distinct and strictly ordered (index {index})")]
    NonCanonicalCapabilityOrder { index: usize },
}
/// Errors raised for lane commitments.
#[derive(Debug, Error)]
pub enum LaneCommitmentError {
    #[error("lane identifier must not be empty")]
    EmptyLaneId,
    #[error("lane identifier contains invalid characters: {lane_id}")]
    InvalidLaneId { lane_id: String },
    #[error("lane identifier must be canonical and at most {maximum} bytes (found {found})")]
    NonCanonicalLaneId { found: usize, maximum: usize },
    #[error("lane capacity must be positive")]
    ZeroCapacity,
    #[error("lane capacity {lane_gib} GiB exceeds declared total {declared_gib} GiB")]
    CapacityExceedsDeclaration { lane_gib: u64, declared_gib: u64 },
}
/// Errors raised for pricing schedule validation.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PricingScheduleError {
    #[error("currency code must not be empty")]
    EmptyCurrency,
    #[error("currency code must be 3-6 lowercase alphanumeric characters")]
    InvalidCurrencyLength,
    #[error("currency code contains invalid characters")]
    InvalidCurrencyChars,
    #[error("currency code must not contain surrounding whitespace")]
    NonCanonicalCurrency,
    #[error("pricing rate must be positive")]
    ZeroRate,
    #[error("minimum commitment must be positive when present")]
    InvalidMinCommitment,
    #[error("notes must not be empty when provided")]
    InvalidNotes,
}
/// Errors raised for metadata validation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MetadataError {
    #[error("metadata key must not be empty")]
    EmptyKey,
    #[error("metadata key contains invalid characters: {key}")]
    InvalidKey { key: String },
    #[error("metadata key must not contain surrounding whitespace")]
    NonCanonicalKey,
    #[error("metadata key has {found} bytes; maximum is {maximum}")]
    KeyTooLong { found: usize, maximum: usize },
    #[error("metadata value must not be empty")]
    EmptyValue,
    #[error(
        "metadata value must be canonical control-free UTF-8 of at most {maximum} bytes (found {found})"
    )]
    InvalidValue { found: usize, maximum: usize },
}
/// Errors raised when validating replication orders.
#[derive(Debug, Error)]
pub enum ReplicationOrderValidationError {
    #[error("unsupported replication order version: {found}")]
    UnsupportedVersion { found: u8 },
    #[error("order identifier must be non-zero")]
    InvalidOrderId,
    #[error("manifest CID has {found} bytes; expected 1..={maximum}")]
    InvalidManifestCidLength { found: usize, maximum: usize },
    #[error("manifest CID must not be all zero")]
    InertManifestCid,
    #[error("manifest CID is not canonical: {reason}")]
    MalformedManifestCid { reason: String },
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    #[error("unknown chunker handle: {handle}")]
    UnknownChunkerHandle { handle: String },
    #[error("chunker handle has {found} bytes; maximum is {maximum}")]
    ProfileHandleTooLong { found: usize, maximum: usize },
    #[error("chunker handle must be canonical (provided {provided}, canonical {canonical})")]
    NonCanonicalProfileHandle { provided: String, canonical: String },
    #[error("target replicas must be at least one")]
    ZeroReplicaTarget,
    #[error("replication order must contain at least one assignment")]
    MissingAssignments,
    #[error("replication order has {found} assignments; maximum is {maximum}")]
    TooManyAssignments { found: usize, maximum: usize },
    #[error("target replicas {target} exceed assignments {assignments}")]
    ReplicaTargetExceedsAssignments { target: u16, assignments: u16 },
    #[error("replication deadline must be greater than issued_at")]
    InvalidDeadline,
    #[error("assignment {index} invalid: {source}")]
    AssignmentInvalid {
        index: usize,
        source: AssignmentError,
    },
    #[error("replication-order providers must be distinct and strictly ascending (index {index})")]
    NonCanonicalProviderOrder { index: usize },
    #[error("SLA invalid: {0}")]
    SlaInvalid(SlaError),
    #[error("SLA ingestion deadline exceeds the replication-order time window")]
    SlaExceedsOrderWindow,
    #[error("replication order has {found} metadata entries; maximum is {maximum}")]
    TooManyMetadataEntries { found: usize, maximum: usize },
    #[error("metadata entry {index} invalid: {source}")]
    MetadataInvalid { index: usize, source: MetadataError },
    #[error("replication-order metadata contains {found} bytes; maximum is {maximum}")]
    MetadataBytesExceeded { found: usize, maximum: usize },
    #[error("duplicate replication-order metadata key `{key}`")]
    DuplicateMetadataKey { key: String },
}
/// Errors raised when validating signed replication-order envelopes.
#[derive(Debug, Error)]
pub enum SignedReplicationOrderValidationError {
    /// Signed replication order version is not supported by this validator.
    #[error("unsupported signed replication order version: {found}")]
    UnsupportedVersion {
        /// Observed signed-envelope version.
        found: u8,
    },
    /// Embedded replication order failed validation.
    #[error("replication order validation failed: {0}")]
    Order(ReplicationOrderValidationError),
    /// Signature key material or signature bytes are empty.
    #[error("replication order signature missing key or signature bytes")]
    InvalidSignature,
}
/// Errors raised while verifying a signed replication-order signature.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplicationOrderSignatureVerificationError {
    /// Signature algorithm is not supported by this validator.
    #[error("unsupported replication order signature algorithm: {0:?}")]
    UnsupportedAlgorithm(SignatureAlgorithm),
    /// Ed25519 public key length is invalid.
    #[error("ed25519 replication order public key must be 32 bytes, got {length}")]
    InvalidPublicKeyLength {
        /// Observed public key byte length.
        length: usize,
    },
    /// Ed25519 signature length is invalid.
    #[error("ed25519 replication order signature must be 64 bytes, got {length}")]
    InvalidSignatureLength {
        /// Observed signature byte length.
        length: usize,
    },
    /// Public key bytes could not be parsed.
    #[error("invalid replication order ed25519 public key: {reason}")]
    InvalidPublicKey {
        /// Underlying parser diagnostic.
        reason: String,
    },
    /// Canonical signature payload could not be encoded.
    #[error("failed to encode replication order signature payload: {reason}")]
    PayloadEncoding {
        /// Underlying Norito diagnostic.
        reason: String,
    },
    /// Signature verification failed.
    #[error("replication order signature verification failed: {reason}")]
    Verification {
        /// Underlying Ed25519 diagnostic.
        reason: String,
    },
}
/// Errors raised for assignment validation.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum AssignmentError {
    #[error("assignment provider identifier must be non-zero")]
    InvalidProviderId,
    #[error("assignment slice must be positive")]
    ZeroSlice,
    #[error("assignment lane hint must not be empty")]
    InvalidLane,
}
/// Errors raised for SLA validation.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SlaError {
    #[error("ingest deadline must be positive")]
    InvalidIngestDeadline,
    #[error("availability percentage out of range: {value}")]
    AvailabilityOutOfRange { value: u32 },
    #[error("proof-of-replication percentage out of range: {value}")]
    PorOutOfRange { value: u32 },
}
/// Errors raised for telemetry validation.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CapacityTelemetryValidationError {
    #[error("unsupported telemetry version: {found}")]
    UnsupportedVersion { found: u8 },
    #[error("provider identifier must be non-zero")]
    InvalidProviderId,
    #[error("epoch_end must be greater than epoch_start")]
    InvalidEpochRange,
    #[error("utilised capacity {utilised_gib} GiB exceeds declared {declared_gib} GiB")]
    UtilisationExceedsDeclaration {
        utilised_gib: u64,
        declared_gib: u64,
    },
    #[error("{field} value {value} out of range (0..=100000)")]
    PercentOutOfRange { field: &'static str, value: u32 },
    #[error("notes must not be empty when provided")]
    InvalidNotes,
}
/// Errors raised while validating dispute payloads.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CapacityDisputeValidationError {
    #[error("unsupported dispute version: {found}")]
    UnsupportedVersion { found: u8 },
    #[error("provider identifier must be non-zero")]
    InvalidProviderId,
    #[error("complainant identifier must be non-zero")]
    InvalidComplainantId,
    #[error("replication order identifier must be non-zero when provided")]
    InvalidReplicationOrderId,
    #[error("description must be 1..={max} characters (got {len})")]
    InvalidDescription { len: usize, max: usize },
    #[error("requested remedy must be 1..={max} characters (got {len})")]
    InvalidRemedy { len: usize, max: usize },
    #[error("evidence invalid: {0}")]
    Evidence(#[from] CapacityDisputeEvidenceError),
}
/// Errors raised while validating dispute evidence metadata.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CapacityDisputeEvidenceError {
    #[error("evidence digest must be non-zero")]
    DigestZero,
    #[error("evidence media type must be 1..={max} characters (got {len})")]
    MediaTypeLength { len: usize, max: usize },
    #[error("evidence URI must be 1..={max} characters (got {len})")]
    UriLength { len: usize, max: usize },
    #[error("evidence size must be positive")]
    SizeNonPositive,
}
fn validate_aliases(
    aliases: &[String],
    descriptor: &'static chunker_registry::ChunkerProfileDescriptor,
) -> Result<(), ChunkerAliasError> {
    if aliases.is_empty() {
        return Err(ChunkerAliasError::InvalidProfileAliases);
    }
    if aliases.iter().any(|alias| alias.trim() != alias) {
        return Err(ChunkerAliasError::InvalidProfileAliases);
    }
    let expected: BTreeSet<_> = descriptor
        .aliases
        .iter()
        .map(|alias| alias.to_string())
        .collect();
    let provided: BTreeSet<_> = aliases.iter().cloned().collect();
    if !provided.iter().all(|alias| expected.contains(alias)) {
        let unknown = provided
            .iter()
            .find(|alias| !expected.contains(*alias))
            .expect("unknown alias must exist");
        return Err(ChunkerAliasError::UnknownProfileAlias {
            alias: unknown.clone(),
        });
    }
    if !provided.contains(descriptor.aliases[0]) {
        return Err(ChunkerAliasError::MissingCanonicalAlias {
            canonical: descriptor.aliases[0].to_owned(),
        });
    }
    Ok(())
}
#[derive(Debug, Error)]
enum ChunkerAliasError {
    #[error("chunker alias list must be non-empty and whitespace-trimmed")]
    InvalidProfileAliases,
    #[error("chunker alias {alias} not recognised by registry")]
    UnknownProfileAlias { alias: String },
    #[error("chunker aliases must include canonical handle {canonical}")]
    MissingCanonicalAlias { canonical: String },
}
impl From<ChunkerAliasError> for ChunkerCommitmentError {
    fn from(value: ChunkerAliasError) -> Self {
        match value {
            ChunkerAliasError::InvalidProfileAliases => {
                ChunkerCommitmentError::InvalidProfileAliases
            }
            ChunkerAliasError::UnknownProfileAlias { alias } => {
                ChunkerCommitmentError::UnknownProfileAlias { alias }
            }
            ChunkerAliasError::MissingCanonicalAlias { canonical } => {
                ChunkerCommitmentError::MissingCanonicalAlias { canonical }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    const SMALL_ORDER_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    fn provider_id(seed: u8) -> [u8; 32] {
        [seed; 32]
    }
    fn base_replication_order() -> ReplicationOrderV1 {
        ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id: [0xAB; 32],
            manifest_cid: crate::canonical_manifest_root_cid([0x11; 32]),
            manifest_digest: [0x10; 32],
            chunking_profile: "sorafs.sf1@1.0.0".to_owned(),
            target_replicas: 2,
            assignments: vec![
                ReplicationAssignmentV1 {
                    provider_id: provider_id(0x01),
                    slice_gib: 50,
                    lane: Some("global".to_owned()),
                },
                ReplicationAssignmentV1 {
                    provider_id: provider_id(0x02),
                    slice_gib: 50,
                    lane: None,
                },
            ],
            issued_at: 1_700_000_000,
            deadline_at: 1_700_000_600,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 300,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 98_000,
            },
            metadata: vec![],
        }
    }
    fn sign_replication_order(
        order: ReplicationOrderV1,
        seed: &[u8; 32],
    ) -> SignedReplicationOrderV1 {
        let signing_key = SigningKey::from_bytes(seed);
        let mut envelope = SignedReplicationOrderV1 {
            version: SIGNED_REPLICATION_ORDER_VERSION_V1,
            order,
            signature: ReplicationOrderSignatureV1 {
                algorithm: SignatureAlgorithm::Ed25519,
                public_key: signing_key.verifying_key().to_bytes().to_vec(),
                signature: vec![0; 64],
            },
        };
        let payload_bytes = envelope
            .signature_payload_bytes()
            .expect("encode replication order signing payload");
        let signature = signing_key.sign(&payload_bytes);
        envelope.signature.signature = signature.to_bytes().to_vec();
        envelope
    }
    fn base_declaration() -> CapacityDeclarationV1 {
        CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: provider_id(0x11),
            stake: StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: crate::deal::XorQuantity::try_from_micro(5_000)
                    .expect("fixture stake is representable"),
            },
            committed_capacity_gib: 500,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".to_owned(),
                profile_aliases: Some(vec!["sorafs.sf1@1.0.0".to_owned(), "sorafs-sf1".to_owned()]),
                committed_gib: 500,
                capability_refs: vec![CapabilityType::ToriiGateway],
            }],
            lane_commitments: vec![],
            pricing: None,
            valid_from: 1_700_000_000,
            valid_until: 1_700_086_400,
            metadata: vec![],
        }
    }
    fn sf2_commitment(committed_gib: u64) -> ChunkerCommitmentV1 {
        ChunkerCommitmentV1 {
            profile_id: "sorafs.sf2@1.0.0".to_owned(),
            profile_aliases: Some(vec!["sorafs.sf2@1.0.0".to_owned(), "sorafs-sf2".to_owned()]),
            committed_gib,
            capability_refs: vec![],
        }
    }
    fn base_dispute() -> CapacityDisputeV1 {
        CapacityDisputeV1 {
            version: CAPACITY_DISPUTE_VERSION_V1,
            provider_id: provider_id(0x21),
            complainant_id: provider_id(0xEE),
            replication_order_id: Some([0xAB; 32]),
            kind: CapacityDisputeKind::ReplicationShortfall,
            evidence: CapacityDisputeEvidenceV1 {
                evidence_digest: [0x55; 32],
                media_type: Some("application/zip".to_owned()),
                uri: Some("https://evidence.example.com/bundle.zip".to_owned()),
                size_bytes: Some(512),
            },
            submitted_epoch: 1_700_100_000,
            description: "Provider failed to ingest replication order within SLA.".to_owned(),
            requested_remedy: Some("Slash 10% of committed stake".to_owned()),
        }
    }
    #[test]
    fn declaration_validation_succeeds_for_well_formed_payload() {
        let mut declaration = base_declaration();
        declaration.pricing = Some(PricingScheduleV1 {
            currency: "xor".to_owned(),
            rate_per_gib_hour_milliu: 12,
            min_commitment_hours: Some(24),
            notes: Some("primary capacity tranche".to_owned()),
        });
        declaration.lane_commitments.push(LaneCommitmentV1 {
            lane_id: "global".to_owned(),
            max_gib: 500,
        });
        declaration.metadata.push(CapacityMetadataEntry {
            key: "region".to_owned(),
            value: "global".to_owned(),
        });
        assert!(declaration.validate().is_ok());
    }
    #[test]
    fn declaration_rejects_non_canonical_profile_handle() {
        let mut declaration = base_declaration();
        declaration.chunker_commitments[0].profile_id = "sorafs/sf1@1.0.0".to_owned();
        let err = declaration.validate().unwrap_err();
        assert!(matches!(
            err,
            CapacityDeclarationValidationError::ChunkerCommitmentInvalid { .. }
        ));
    }
    #[test]
    fn declaration_rejects_duplicate_profile_commitment() {
        let mut declaration = base_declaration();
        declaration.chunker_commitments.push(ChunkerCommitmentV1 {
            profile_id: "sorafs.sf1@1.0.0".to_owned(),
            profile_aliases: Some(vec!["sorafs.sf1@1.0.0".to_owned(), "sorafs-sf1".to_owned()]),
            committed_gib: 100,
            capability_refs: vec![],
        });
        let err = declaration.validate().unwrap_err();
        assert!(matches!(
            err,
            CapacityDeclarationValidationError::NonCanonicalChunkerOrder { .. }
        ));
    }
    #[test]
    fn declaration_rejects_inert_stake_and_unaccounted_capacity() {
        let mut inert_pool = base_declaration();
        inert_pool.stake.pool_id = [0; 32];
        assert!(matches!(
            inert_pool.validate(),
            Err(CapacityDeclarationValidationError::InvalidStakePoolId)
        ));
        let mut undercommitted = base_declaration();
        undercommitted.committed_capacity_gib += 1;
        assert!(matches!(
            undercommitted.validate(),
            Err(
                CapacityDeclarationValidationError::CommittedCapacityMismatch {
                    committed_gib: 500,
                    declared_gib: 501,
                }
            )
        ));
        let mut overcommitted = base_declaration();
        overcommitted.committed_capacity_gib -= 1;
        assert!(matches!(
            overcommitted.validate(),
            Err(
                CapacityDeclarationValidationError::CommittedCapacityMismatch {
                    committed_gib: 500,
                    declared_gib: 499,
                }
            )
        ));
    }
    #[test]
    fn declaration_rejects_noncanonical_collection_order() {
        let mut reversed_profiles = base_declaration();
        reversed_profiles.chunker_commitments[0].committed_gib = 250;
        reversed_profiles
            .chunker_commitments
            .insert(0, sf2_commitment(250));
        assert!(matches!(
            reversed_profiles.validate(),
            Err(CapacityDeclarationValidationError::NonCanonicalChunkerOrder { .. })
        ));
        let mut reversed_lanes = base_declaration();
        reversed_lanes.lane_commitments = vec![
            LaneCommitmentV1 {
                lane_id: "warm".to_owned(),
                max_gib: 250,
            },
            LaneCommitmentV1 {
                lane_id: "hot".to_owned(),
                max_gib: 250,
            },
        ];
        assert!(matches!(
            reversed_lanes.validate(),
            Err(CapacityDeclarationValidationError::NonCanonicalLaneOrder { .. })
        ));
        let mut duplicate_lanes = base_declaration();
        duplicate_lanes.lane_commitments = vec![
            LaneCommitmentV1 {
                lane_id: "hot".to_owned(),
                max_gib: 250,
            },
            LaneCommitmentV1 {
                lane_id: "hot".to_owned(),
                max_gib: 250,
            },
        ];
        assert!(matches!(
            duplicate_lanes.validate(),
            Err(CapacityDeclarationValidationError::NonCanonicalLaneOrder { .. })
        ));
        let mut duplicate_capabilities = base_declaration();
        duplicate_capabilities.chunker_commitments[0].capability_refs =
            vec![CapabilityType::ToriiGateway, CapabilityType::ToriiGateway];
        assert!(matches!(
            duplicate_capabilities.validate(),
            Err(
                CapacityDeclarationValidationError::ChunkerCommitmentInvalid {
                    source: ChunkerCommitmentError::NonCanonicalCapabilityOrder { .. },
                    ..
                }
            )
        ));
        let mut reversed_capabilities = base_declaration();
        reversed_capabilities.chunker_commitments[0].capability_refs =
            vec![CapabilityType::QuicNoise, CapabilityType::ToriiGateway];
        assert!(matches!(
            reversed_capabilities.validate(),
            Err(
                CapacityDeclarationValidationError::ChunkerCommitmentInvalid {
                    source: ChunkerCommitmentError::NonCanonicalCapabilityOrder { .. },
                    ..
                }
            )
        ));
    }
    #[test]
    fn declaration_rejects_resource_exhaustion_shapes() {
        let mut chunker_flood = base_declaration();
        chunker_flood.chunker_commitments = vec![
            chunker_flood.chunker_commitments[0].clone();
            MAX_CAPACITY_CHUNKER_COMMITMENTS + 1
        ];
        assert!(matches!(
            chunker_flood.validate(),
            Err(CapacityDeclarationValidationError::TooManyChunkerCommitments { .. })
        ));
        let mut lane_flood = base_declaration();
        lane_flood.lane_commitments = (0..=MAX_CAPACITY_LANE_COMMITMENTS)
            .map(|index| LaneCommitmentV1 {
                lane_id: format!("lane-{index:02}"),
                max_gib: 1,
            })
            .collect();
        assert!(matches!(
            lane_flood.validate(),
            Err(CapacityDeclarationValidationError::TooManyLaneCommitments { .. })
        ));
        let mut metadata_flood = base_declaration();
        metadata_flood.metadata = (0..=MAX_CAPACITY_DECLARATION_METADATA_ENTRIES)
            .map(|index| CapacityMetadataEntry {
                key: format!("key{index}"),
                value: "value".to_owned(),
            })
            .collect();
        assert!(matches!(
            metadata_flood.validate(),
            Err(CapacityDeclarationValidationError::TooManyMetadataEntries { .. })
        ));
        let mut metadata_bytes = base_declaration();
        metadata_bytes.metadata = (0..MAX_CAPACITY_DECLARATION_METADATA_ENTRIES)
            .map(|index| CapacityMetadataEntry {
                key: format!("key{index}"),
                value: "x".repeat(1024),
            })
            .collect();
        assert!(matches!(
            metadata_bytes.validate(),
            Err(CapacityDeclarationValidationError::MetadataBytesExceeded { .. })
        ));
        let mut alias_flood = base_declaration();
        alias_flood.chunker_commitments[0].profile_aliases = Some(
            (0..=MAX_CAPACITY_PROFILE_ALIASES)
                .map(|_| "sorafs.sf1@1.0.0".to_owned())
                .collect(),
        );
        assert!(matches!(
            alias_flood.validate(),
            Err(
                CapacityDeclarationValidationError::ChunkerCommitmentInvalid {
                    source: ChunkerCommitmentError::InvalidProfileAliasCount { .. },
                    ..
                }
            )
        ));
        let mut capability_flood = base_declaration();
        capability_flood.chunker_commitments[0].capability_refs =
            vec![CapabilityType::ToriiGateway; MAX_CAPACITY_CAPABILITY_REFS + 1];
        assert!(matches!(
            capability_flood.validate(),
            Err(
                CapacityDeclarationValidationError::ChunkerCommitmentInvalid {
                    source: ChunkerCommitmentError::TooManyCapabilities { .. },
                    ..
                }
            )
        ));
    }
    #[test]
    fn declaration_rejects_oversized_and_noncanonical_text() {
        let mut profile = base_declaration();
        profile.chunker_commitments[0].profile_id =
            "x".repeat(MAX_CAPACITY_PROFILE_HANDLE_BYTES + 1);
        assert!(matches!(
            profile.validate(),
            Err(
                CapacityDeclarationValidationError::ChunkerCommitmentInvalid {
                    source: ChunkerCommitmentError::ProfileHandleTooLong { .. },
                    ..
                }
            )
        ));
        let mut alias = base_declaration();
        alias.chunker_commitments[0].profile_aliases = Some(vec![
            "sorafs.sf1@1.0.0".to_owned(),
            "x".repeat(MAX_CAPACITY_PROFILE_HANDLE_BYTES + 1),
        ]);
        assert!(matches!(
            alias.validate(),
            Err(
                CapacityDeclarationValidationError::ChunkerCommitmentInvalid {
                    source: ChunkerCommitmentError::ProfileAliasTooLong { .. },
                    ..
                }
            )
        ));
        let mut alias_order = base_declaration();
        alias_order.chunker_commitments[0]
            .profile_aliases
            .as_mut()
            .expect("base declaration has aliases")
            .reverse();
        assert!(matches!(
            alias_order.validate(),
            Err(
                CapacityDeclarationValidationError::ChunkerCommitmentInvalid {
                    source: ChunkerCommitmentError::NonCanonicalProfileAliases,
                    ..
                }
            )
        ));
        let mut padded_lane = base_declaration();
        padded_lane.lane_commitments = vec![LaneCommitmentV1 {
            lane_id: " hot ".to_owned(),
            max_gib: 1,
        }];
        assert!(matches!(
            padded_lane.validate(),
            Err(CapacityDeclarationValidationError::LaneCommitmentInvalid {
                source: LaneCommitmentError::NonCanonicalLaneId { .. },
                ..
            })
        ));
        let mut oversized_lane = base_declaration();
        oversized_lane.lane_commitments = vec![LaneCommitmentV1 {
            lane_id: "x".repeat(MAX_CAPACITY_LANE_ID_BYTES + 1),
            max_gib: 1,
        }];
        assert!(matches!(
            oversized_lane.validate(),
            Err(CapacityDeclarationValidationError::LaneCommitmentInvalid {
                source: LaneCommitmentError::NonCanonicalLaneId { .. },
                ..
            })
        ));
        let mut padded_currency = base_declaration();
        padded_currency.pricing = Some(PricingScheduleV1 {
            currency: " xor ".to_owned(),
            rate_per_gib_hour_milliu: 1,
            min_commitment_hours: None,
            notes: None,
        });
        assert!(matches!(
            padded_currency.validate(),
            Err(CapacityDeclarationValidationError::PricingInvalid(
                PricingScheduleError::NonCanonicalCurrency
            ))
        ));
        let mut zero_rate = base_declaration();
        zero_rate.pricing = Some(PricingScheduleV1 {
            currency: "xor".to_owned(),
            rate_per_gib_hour_milliu: 0,
            min_commitment_hours: None,
            notes: None,
        });
        assert!(matches!(
            zero_rate.validate(),
            Err(CapacityDeclarationValidationError::PricingInvalid(
                PricingScheduleError::ZeroRate
            ))
        ));
        let mut oversized_notes = base_declaration();
        oversized_notes.pricing = Some(PricingScheduleV1 {
            currency: "xor".to_owned(),
            rate_per_gib_hour_milliu: 1,
            min_commitment_hours: None,
            notes: Some("x".repeat(MAX_CAPACITY_PRICING_NOTES_BYTES + 1)),
        });
        assert!(matches!(
            oversized_notes.validate(),
            Err(CapacityDeclarationValidationError::PricingInvalid(
                PricingScheduleError::InvalidNotes
            ))
        ));
        let mut duplicate_metadata = base_declaration();
        duplicate_metadata.metadata = vec![
            CapacityMetadataEntry {
                key: "region".to_owned(),
                value: "one".to_owned(),
            },
            CapacityMetadataEntry {
                key: "region".to_owned(),
                value: "two".to_owned(),
            },
        ];
        assert!(matches!(
            duplicate_metadata.validate(),
            Err(CapacityDeclarationValidationError::DuplicateMetadataKey { .. })
        ));
        let mut oversized_metadata = base_declaration();
        oversized_metadata.metadata = vec![CapacityMetadataEntry {
            key: "region".to_owned(),
            value: "x".repeat(MAX_CAPACITY_METADATA_VALUE_BYTES + 1),
        }];
        assert!(matches!(
            oversized_metadata.validate(),
            Err(CapacityDeclarationValidationError::MetadataInvalid {
                source: MetadataError::InvalidValue { .. },
                ..
            })
        ));
    }
    #[test]
    fn replication_order_validation_checks_assignments() {
        let order = base_replication_order();
        assert!(order.validate().is_ok());
    }
    #[test]
    fn signed_replication_order_payload_excludes_signature() {
        let envelope = sign_replication_order(base_replication_order(), &[0xA7; 32]);
        let payload_bytes = envelope
            .signature_payload_bytes()
            .expect("encode signed order payload");
        let payload: ReplicationOrderSigningPayloadV1 =
            norito::decode_from_bytes(&payload_bytes).expect("decode signing payload");
        assert_eq!(payload.domain, REPLICATION_ORDER_SIGNATURE_DOMAIN_V1);
        let mut different_signature = envelope.clone();
        different_signature.signature.signature = vec![0xBB; 64];
        assert_eq!(
            payload_bytes,
            different_signature
                .signature_payload_bytes()
                .expect("encode signed order payload")
        );
        let mut different_order = envelope.clone();
        different_order.order.deadline_at += 1;
        assert_ne!(
            envelope
                .signature_payload_bytes()
                .expect("encode signed order payload"),
            different_order
                .signature_payload_bytes()
                .expect("encode signed order payload")
        );
    }
    #[test]
    fn signed_replication_order_verifies_ed25519_signature() {
        let envelope = sign_replication_order(base_replication_order(), &[0xA7; 32]);
        envelope
            .verify_signature()
            .expect("signed replication order verifies");
    }
    #[test]
    fn signed_replication_order_rejects_tampered_order() {
        let mut envelope = sign_replication_order(base_replication_order(), &[0xA7; 32]);
        envelope.order.deadline_at += 1;
        assert!(matches!(
            envelope.verify_signature(),
            Err(ReplicationOrderSignatureVerificationError::Verification { .. })
        ));
    }
    #[test]
    fn signed_replication_order_rejects_all_zero_signature_material() {
        let mut envelope = sign_replication_order(base_replication_order(), &[0xA7; 32]);
        envelope.signature.signature.fill(0);
        let err = envelope
            .verify_signature()
            .expect_err("all-zero replication order signature must be rejected");
        assert!(matches!(
            err,
            ReplicationOrderSignatureVerificationError::Verification { reason }
                if reason.contains("all zero")
        ));
    }
    #[test]
    fn signed_replication_order_rejects_malformed_ed25519_signature_r() {
        for (label, replacement_r, expected_reason) in [
            ("small-order", SMALL_ORDER_R, "small-order"),
            ("noncanonical", NONCANONICAL_R, "not a canonical"),
        ] {
            let mut envelope = sign_replication_order(base_replication_order(), &[0xA7; 32]);
            envelope.signature.signature[..PUBLIC_KEY_LENGTH].copy_from_slice(&replacement_r);
            let err = envelope
                .verify_signature()
                .expect_err("malformed replication order signature R must be rejected");
            assert!(
                matches!(
                    &err,
                    ReplicationOrderSignatureVerificationError::Verification { reason }
                        if reason.contains(expected_reason)
                ),
                "{label} signature R produced unexpected error: {err}"
            );
        }
    }
    #[test]
    fn signed_replication_order_validate_rejects_all_zero_signature_material() {
        let mut envelope = sign_replication_order(base_replication_order(), &[0xA7; 32]);
        envelope.signature.signature.fill(0);
        assert!(matches!(
            envelope.validate(),
            Err(SignedReplicationOrderValidationError::InvalidSignature)
        ));
        envelope.signature.signature = vec![0xA7; 64];
        envelope.signature.public_key = vec![0; 32];
        assert!(matches!(
            envelope.validate(),
            Err(SignedReplicationOrderValidationError::InvalidSignature)
        ));
    }
    #[test]
    fn replication_order_rejects_duplicate_providers() {
        let order = ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id: [0xAB; 32],
            manifest_cid: crate::canonical_manifest_root_cid([0x11; 32]),
            manifest_digest: [0x10; 32],
            chunking_profile: "sorafs.sf1@1.0.0".to_owned(),
            target_replicas: 1,
            assignments: vec![
                ReplicationAssignmentV1 {
                    provider_id: provider_id(0x01),
                    slice_gib: 50,
                    lane: None,
                },
                ReplicationAssignmentV1 {
                    provider_id: provider_id(0x01),
                    slice_gib: 25,
                    lane: None,
                },
            ],
            issued_at: 1_700_000_000,
            deadline_at: 1_700_000_600,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 300,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 98_000,
            },
            metadata: vec![],
        };
        let err = order.validate().unwrap_err();
        assert!(matches!(
            err,
            ReplicationOrderValidationError::NonCanonicalProviderOrder { .. }
        ));
    }
    #[test]
    fn replication_order_rejects_resource_exhaustion_shapes() {
        let mut oversized_cid = base_replication_order();
        oversized_cid.manifest_cid = vec![1; MAX_REPLICATION_ORDER_MANIFEST_CID_BYTES + 1];
        assert!(matches!(
            oversized_cid.validate(),
            Err(ReplicationOrderValidationError::InvalidManifestCidLength { .. })
        ));
        let mut wrong_version = base_replication_order();
        wrong_version.manifest_cid[0] = 2;
        assert!(matches!(
            wrong_version.validate(),
            Err(ReplicationOrderValidationError::MalformedManifestCid { .. })
        ));
        let mut wrong_codec = base_replication_order();
        wrong_codec.manifest_cid[1] = 0x55;
        assert!(matches!(
            wrong_codec.validate(),
            Err(ReplicationOrderValidationError::MalformedManifestCid { .. })
        ));
        let mut zero_digest = base_replication_order();
        zero_digest.manifest_cid[4..].fill(0);
        assert!(matches!(
            zero_digest.validate(),
            Err(ReplicationOrderValidationError::InertManifestCid)
        ));
        let mut oversized_profile = base_replication_order();
        oversized_profile.chunking_profile = "x".repeat(MAX_REPLICATION_ORDER_PROFILE_BYTES + 1);
        assert!(matches!(
            oversized_profile.validate(),
            Err(ReplicationOrderValidationError::ProfileHandleTooLong { .. })
        ));
        let mut assignment_flood = base_replication_order();
        assignment_flood.assignments = (0..=MAX_REPLICATION_ORDER_ASSIGNMENTS)
            .map(|index| {
                let mut provider_id = [0u8; 32];
                provider_id[28..].copy_from_slice(
                    &u32::try_from(index + 1)
                        .expect("test index fits u32")
                        .to_be_bytes(),
                );
                ReplicationAssignmentV1 {
                    provider_id,
                    slice_gib: 1,
                    lane: None,
                }
            })
            .collect();
        assert!(matches!(
            assignment_flood.validate(),
            Err(ReplicationOrderValidationError::TooManyAssignments { .. })
        ));
        let mut metadata_flood = base_replication_order();
        metadata_flood.metadata = (0..=MAX_REPLICATION_ORDER_METADATA_ENTRIES)
            .map(|index| CapacityMetadataEntry {
                key: format!("key{index}"),
                value: "value".to_owned(),
            })
            .collect();
        assert!(matches!(
            metadata_flood.validate(),
            Err(ReplicationOrderValidationError::TooManyMetadataEntries { .. })
        ));
    }
    #[test]
    fn replication_order_rejects_noncanonical_assignments_sla_and_metadata() {
        let mut reversed = base_replication_order();
        reversed.assignments.reverse();
        assert!(matches!(
            reversed.validate(),
            Err(ReplicationOrderValidationError::NonCanonicalProviderOrder { .. })
        ));
        let mut invalid_lane = base_replication_order();
        invalid_lane.assignments[0].lane = Some(" Global ".to_owned());
        assert!(matches!(
            invalid_lane.validate(),
            Err(ReplicationOrderValidationError::AssignmentInvalid { .. })
        ));
        let mut zero_sla = base_replication_order();
        zero_sla.sla.min_availability_percent_milli = 0;
        assert!(matches!(
            zero_sla.validate(),
            Err(ReplicationOrderValidationError::SlaInvalid(_))
        ));
        let mut long_sla = base_replication_order();
        long_sla.sla.ingest_deadline_secs = 601;
        assert!(matches!(
            long_sla.validate(),
            Err(ReplicationOrderValidationError::SlaExceedsOrderWindow)
        ));
        let mut duplicate_metadata = base_replication_order();
        duplicate_metadata.metadata = vec![
            CapacityMetadataEntry {
                key: "region".to_owned(),
                value: "one".to_owned(),
            },
            CapacityMetadataEntry {
                key: "region".to_owned(),
                value: "two".to_owned(),
            },
        ];
        assert!(matches!(
            duplicate_metadata.validate(),
            Err(ReplicationOrderValidationError::DuplicateMetadataKey { .. })
        ));
    }
    #[test]
    fn telemetry_validation_enforces_ranges() {
        let telemetry = CapacityTelemetryV1 {
            version: CAPACITY_TELEMETRY_VERSION_V1,
            provider_id: provider_id(0x33),
            epoch_start: 1_700_000_000,
            epoch_end: 1_700_000_360,
            declared_capacity_gib: 400,
            utilised_capacity_gib: 399,
            successful_replications: 10,
            failed_replications: 1,
            uptime_percent_milli: 99_500,
            por_success_percent_milli: 99_000,
            notes: None,
        };
        assert!(telemetry.validate().is_ok());
        let telemetry_overflow = CapacityTelemetryV1 {
            utilised_capacity_gib: 401,
            ..telemetry
        };
        let err = telemetry_overflow.validate().unwrap_err();
        assert!(matches!(
            err,
            CapacityTelemetryValidationError::UtilisationExceedsDeclaration { .. }
        ));
    }
    #[test]
    fn dispute_validation_accepts_well_formed_payload() {
        let dispute = base_dispute();
        assert!(dispute.validate().is_ok());
    }
    #[test]
    fn dispute_validation_rejects_empty_description() {
        let mut dispute = base_dispute();
        dispute.description = "   ".to_owned();
        let err = dispute.validate().unwrap_err();
        assert!(matches!(
            err,
            CapacityDisputeValidationError::InvalidDescription { .. }
        ));
    }
    #[test]
    fn dispute_validation_rejects_zero_evidence_digest() {
        let mut dispute = base_dispute();
        dispute.evidence.evidence_digest = [0u8; 32];
        let err = dispute.validate().unwrap_err();
        assert!(matches!(
            err,
            CapacityDisputeValidationError::Evidence(CapacityDisputeEvidenceError::DigestZero)
        ));
    }
}
