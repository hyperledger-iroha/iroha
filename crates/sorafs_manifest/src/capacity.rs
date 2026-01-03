#![allow(unexpected_cfgs)]

//! Capacity marketplace schemas and validators for SoraFS (SF-2c).
//!
//! The Norito payloads defined here allow governance, Torii, and storage
//! operators to exchange deterministic capacity declarations, replication
//! orders, and telemetry snapshots.

use std::collections::{BTreeSet, HashSet};

use norito::{
    core::{DecodeFromSlice, decode_field_canonical},
    derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize},
};
use thiserror::Error;

use crate::{
    chunker_registry,
    provider_advert::{CapabilityType, StakePointer},
};

/// Schema version for [`CapacityDeclarationV1`].
pub const CAPACITY_DECLARATION_VERSION_V1: u8 = 1;
/// Schema version for [`ReplicationOrderV1`].
pub const REPLICATION_ORDER_VERSION_V1: u8 = 1;
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
        if self.committed_capacity_gib == 0 {
            return Err(CapacityDeclarationValidationError::ZeroCommittedCapacity);
        }
        if self.chunker_commitments.is_empty() {
            return Err(CapacityDeclarationValidationError::MissingChunkerCommitments);
        }
        if self.valid_until <= self.valid_from {
            return Err(CapacityDeclarationValidationError::InvalidValidityWindow);
        }

        let mut seen_profiles: HashSet<String> = HashSet::new();
        let mut total_committed = 0u128;
        for (index, commitment) in self.chunker_commitments.iter().enumerate() {
            commitment.validate().map_err(|source| {
                CapacityDeclarationValidationError::ChunkerCommitmentInvalid { index, source }
            })?;
            let canonical = commitment.profile_id.clone();
            if !seen_profiles.insert(canonical.clone()) {
                return Err(
                    CapacityDeclarationValidationError::DuplicateChunkerCommitment {
                        handle: canonical,
                    },
                );
            }
            total_committed = total_committed
                .checked_add(u128::from(commitment.committed_gib))
                .ok_or(CapacityDeclarationValidationError::CommittedCapacityOverflow)?;
        }
        if total_committed > u128::from(self.committed_capacity_gib) {
            return Err(
                CapacityDeclarationValidationError::CommittedCapacityExceedsDeclaration {
                    committed_gib: total_committed,
                    declared_gib: self.committed_capacity_gib,
                },
            );
        }

        let mut seen_lanes = HashSet::new();
        for (index, lane) in self.lane_commitments.iter().enumerate() {
            lane.validate(self.committed_capacity_gib)
                .map_err(
                    |source| CapacityDeclarationValidationError::LaneCommitmentInvalid {
                        index,
                        source,
                    },
                )?;
            if !seen_lanes.insert(lane.lane_id.to_owned()) {
                return Err(
                    CapacityDeclarationValidationError::DuplicateLaneCommitment {
                        lane_id: lane.lane_id.clone(),
                    },
                );
            }
        }

        if let Some(pricing) = &self.pricing {
            pricing
                .validate()
                .map_err(CapacityDeclarationValidationError::PricingInvalid)?;
        }

        for (index, entry) in self.metadata.iter().enumerate() {
            entry.validate().map_err(|source| {
                CapacityDeclarationValidationError::MetadataInvalid { index, source }
            })?;
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
            validate_aliases(aliases, descriptor).map_err(ChunkerCommitmentError::from)?;
        }
        let mut seen_caps: HashSet<u16> = HashSet::new();
        for capability in &self.capability_refs {
            let key = *capability as u16;
            if !seen_caps.insert(key) {
                return Err(ChunkerCommitmentError::DuplicateCapability {
                    capability: *capability,
                });
            }
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
        if self.min_commitment_hours == Some(0) {
            return Err(PricingScheduleError::InvalidMinCommitment);
        }
        if let Some(notes) = &self.notes
            && notes.trim().is_empty()
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
        if self.value.trim().is_empty() {
            return Err(MetadataError::EmptyValue);
        }
        Ok(())
    }
}

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
        if self.manifest_cid.is_empty() {
            return Err(ReplicationOrderValidationError::EmptyManifestCid);
        }
        if self.manifest_digest.iter().all(|&byte| byte == 0) {
            return Err(ReplicationOrderValidationError::InvalidManifestDigest);
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

        let mut seen_providers = HashSet::new();
        for (index, assignment) in self.assignments.iter().enumerate() {
            assignment.validate().map_err(|source| {
                ReplicationOrderValidationError::AssignmentInvalid { index, source }
            })?;
            if !seen_providers.insert(assignment.provider_id) {
                return Err(ReplicationOrderValidationError::DuplicateProvider {
                    provider_id: assignment.provider_id,
                });
            }
        }
        self.sla
            .validate()
            .map_err(ReplicationOrderValidationError::SlaInvalid)?;

        for (index, entry) in self.metadata.iter().enumerate() {
            entry.validate().map_err(|source| {
                ReplicationOrderValidationError::MetadataInvalid { index, source }
            })?;
        }

        Ok(())
    }
}

/// Assignment binding a provider to store a manifest slice.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
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
            && lane.trim().is_empty()
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
        if self.min_availability_percent_milli > 100_000 {
            return Err(SlaError::AvailabilityOutOfRange {
                value: self.min_availability_percent_milli,
            });
        }
        if self.min_por_success_percent_milli > 100_000 {
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
    #[error("total committed capacity must be positive")]
    ZeroCommittedCapacity,
    #[error("at least one chunker commitment is required")]
    MissingChunkerCommitments,
    #[error("chunker commitment duplicates profile {handle}")]
    DuplicateChunkerCommitment { handle: String },
    #[error("chunker commitment total {committed_gib} GiB exceeds declared {declared_gib} GiB")]
    CommittedCapacityExceedsDeclaration {
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
    #[error("duplicate lane commitment for {lane_id}")]
    DuplicateLaneCommitment { lane_id: String },
    #[error("pricing schedule invalid: {0}")]
    PricingInvalid(PricingScheduleError),
    #[error("valid_until must be greater than valid_from")]
    InvalidValidityWindow,
    #[error("metadata entry {index} invalid: {source}")]
    MetadataInvalid { index: usize, source: MetadataError },
    #[error("committed capacity overflowed 128-bit accumulator")]
    CommittedCapacityOverflow,
}

/// Errors raised when validating chunker commitments.
#[derive(Debug, Error)]
pub enum ChunkerCommitmentError {
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
    #[error("duplicate capability reference: {capability:?}")]
    DuplicateCapability { capability: CapabilityType },
}

/// Errors raised for lane commitments.
#[derive(Debug, Error)]
pub enum LaneCommitmentError {
    #[error("lane identifier must not be empty")]
    EmptyLaneId,
    #[error("lane identifier contains invalid characters: {lane_id}")]
    InvalidLaneId { lane_id: String },
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
    #[error("metadata value must not be empty")]
    EmptyValue,
}

/// Errors raised when validating replication orders.
#[derive(Debug, Error)]
pub enum ReplicationOrderValidationError {
    #[error("unsupported replication order version: {found}")]
    UnsupportedVersion { found: u8 },
    #[error("order identifier must be non-zero")]
    InvalidOrderId,
    #[error("manifest CID must not be empty")]
    EmptyManifestCid,
    #[error("manifest digest must be non-zero")]
    InvalidManifestDigest,
    #[error("unknown chunker handle: {handle}")]
    UnknownChunkerHandle { handle: String },
    #[error("chunker handle must be canonical (provided {provided}, canonical {canonical})")]
    NonCanonicalProfileHandle { provided: String, canonical: String },
    #[error("target replicas must be at least one")]
    ZeroReplicaTarget,
    #[error("replication order must contain at least one assignment")]
    MissingAssignments,
    #[error("target replicas {target} exceed assignments {assignments}")]
    ReplicaTargetExceedsAssignments { target: u16, assignments: u16 },
    #[error("replication deadline must be greater than issued_at")]
    InvalidDeadline,
    #[error("assignment {index} invalid: {source}")]
    AssignmentInvalid {
        index: usize,
        source: AssignmentError,
    },
    #[error("duplicate provider appearing in assignments")]
    DuplicateProvider { provider_id: [u8; 32] },
    #[error("SLA invalid: {0}")]
    SlaInvalid(SlaError),
    #[error("metadata entry {index} invalid: {source}")]
    MetadataInvalid { index: usize, source: MetadataError },
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

    fn provider_id(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn base_declaration() -> CapacityDeclarationV1 {
        CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: provider_id(0x11),
            stake: StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: 5_000,
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
            CapacityDeclarationValidationError::DuplicateChunkerCommitment { .. }
        ));
    }

    #[test]
    fn replication_order_validation_checks_assignments() {
        let order = ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id: [0xAB; 32],
            manifest_cid: vec![0x01, 0x55],
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
        };
        assert!(order.validate().is_ok());
    }

    #[test]
    fn replication_order_rejects_duplicate_providers() {
        let order = ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id: [0xAB; 32],
            manifest_cid: vec![0x01, 0x55],
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
            ReplicationOrderValidationError::DuplicateProvider { .. }
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
