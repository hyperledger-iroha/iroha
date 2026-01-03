#![allow(unexpected_cfgs)]

//! Norito-encoded manifest model for SoraFS artifacts.
//!
//! The structure tracks the metadata described in the SoraFS Architecture
//! RFC (SF-1): chunking profile, CAR commitments, pin policies, and governance
//! attestations. Encoding uses Norito so manifests can be validated by Torii,
//! gateways, and storage nodes without bespoke parsers.

use blake3::Hash;
use norito::{
    core::Error as NoritoError,
    derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize},
    json::{self, FastJsonWrite, JsonSerialize as NoritoJsonSerialize},
};
use sorafs_chunker::ChunkProfile;
use thiserror::Error;

pub mod alias_cache;
pub mod capacity;
pub mod chunker_registry;
pub mod deal;
pub mod gar;
pub mod gateway;
pub mod gateway_fixture;
pub mod governance;
pub mod hosts;
pub mod hybrid_envelope;
pub mod manifest_capabilities;
pub mod pdp;
pub mod pin_registry;
pub mod por;
pub mod potr;
pub mod pricing;
pub mod proof_stream;
pub mod provider_admission;
pub mod provider_advert;
pub mod repair;
pub mod token;
pub mod validation;

pub use capacity::{
    AssignmentError, CAPACITY_DECLARATION_VERSION_V1, CAPACITY_DISPUTE_VERSION_V1,
    CAPACITY_TELEMETRY_VERSION_V1, CapacityDeclarationV1, CapacityDeclarationValidationError,
    CapacityDisputeEvidenceError, CapacityDisputeEvidenceV1, CapacityDisputeKind,
    CapacityDisputeV1, CapacityDisputeValidationError, CapacityMetadataEntry, CapacityTelemetryV1,
    CapacityTelemetryValidationError, ChunkerCommitmentError, ChunkerCommitmentV1,
    LaneCommitmentError, LaneCommitmentV1, MetadataError, PricingScheduleError, PricingScheduleV1,
    REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1, ReplicationOrderSlaV1,
    ReplicationOrderV1, ReplicationOrderValidationError, SlaError,
};
pub use chunker_registry::{ChunkerProfileDescriptor, DEFAULT_MULTIHASH_CODE, MANIFEST_DAG_CODEC};
pub use deal::{
    BASIS_POINTS_PER_UNIT, DEAL_LEDGER_VERSION_V1, DEAL_MICROPAYMENT_VERSION_V1,
    DEAL_SETTLEMENT_VERSION_V1, DEAL_TERMS_VERSION_V1, DealAmountError, DealLedgerSnapshotV1,
    DealLedgerValidationError, DealMicropaymentV1, DealMicropaymentValidationError,
    DealSettlementStatusV1, DealSettlementV1, DealSettlementValidationError, DealTermsV1,
    DealTermsValidationError, MICRO_XOR_PER_XOR, MicropaymentPolicyError, MicropaymentPolicyV1,
    XorAmount,
};
pub use gateway::{
    GatewayAuthorizationError, GatewayAuthorizationRecord, GatewayAuthorizationVerifier,
    HostPattern,
};
pub use governance::{
    GOVERNANCE_LOG_VERSION_V1, GovernanceLogNodeV1, GovernanceLogPayloadV1,
    GovernanceLogSignatureV1, GovernanceLogValidationError, GovernanceSignatureAlgorithm,
};
pub use hosts::{DirectCarLocator, HostMappingInput, HostMappingSummary};
pub use manifest_capabilities::{
    ChunkProfileSummary, ManifestCapabilitySummary, detect_manifest_capabilities,
};
pub use pdp::{
    HashAlgorithmV1, PDP_CHALLENGE_VERSION_V1, PDP_COMMITMENT_VERSION_V1, PDP_PROOF_VERSION_V1,
    PdpChallengeV1, PdpChallengeValidationError, PdpCommitmentV1, PdpCommitmentValidationError,
    PdpHotLeafProofV1, PdpProofLeafV1, PdpProofV1, PdpProofValidationError, PdpSampleV1,
};
pub use pin_registry::{
    AliasBindingV1, AliasBindingValidationError, ManifestPolicyV1, ManifestPolicyValidationError,
    PinRecordV1, PinRecordValidationError, ReplicationOrderV1 as PinRegistryReplicationOrderV1,
    ReplicationOrderValidationError as PinRegistryReplicationOrderValidationError,
    ReplicationReceiptStatus, ReplicationReceiptV1, ReplicationReceiptValidationError,
};
pub use por::{
    AUDIT_VERDICT_VERSION_V1, AuditOutcomeV1, AuditVerdictV1, AuditVerdictValidationError,
    MANUAL_POR_CHALLENGE_VERSION_V1, ManualPorChallengeV1, ManualPorChallengeValidationError,
    POR_CHALLENGE_STATUS_VERSION_V1, POR_CHALLENGE_VERSION_V1, POR_PROOF_VERSION_V1,
    POR_WEEKLY_REPORT_VERSION_V1, PorChallengeOutcome, PorChallengeOutcomeParseError,
    PorChallengeStatusV1, PorChallengeStatusValidationError, PorChallengeV1,
    PorChallengeValidationError, PorProofSampleV1, PorProofV1, PorProofValidationError,
    PorProviderSummaryV1, PorProviderSummaryValidationError, PorReportIsoWeek,
    PorReportIsoWeekValidationError, PorSlashingEventV1, PorSlashingEventValidationError,
    PorWeeklyReportV1, PorWeeklyReportValidationError,
};
pub use potr::{POTR_RECEIPT_VERSION_V1, PotrReceiptV1, PotrReceiptValidationError, PotrStatus};
pub use pricing::{
    BondPolicyError, BondPolicyV1, CreditPolicyError, CreditPolicyV1, MicropaymentDecision,
    PRICING_MANIFEST_VERSION_V1, PricingManifestError, PricingManifestV1,
    PricingMicropaymentPolicyError, PricingMicropaymentPolicyV1, PricingTierError, PricingTierV1,
};
pub use proof_stream::{
    ProofStreamKind, ProofStreamRequestError, ProofStreamRequestV1, ProofStreamTier,
};
pub use provider_admission::{
    AdmissionRecord, ENDPOINT_ATTESTATION_VERSION_V1, EndpointAdmissionError, EndpointAdmissionV1,
    EndpointAttestationError, EndpointAttestationKind, EndpointAttestationV1,
    PROVIDER_ADMISSION_ENVELOPE_VERSION_V1, PROVIDER_ADMISSION_PROPOSAL_VERSION_V1,
    PROVIDER_ADMISSION_RENEWAL_VERSION_V1, PROVIDER_ADMISSION_REVOCATION_VERSION_V1,
    ProviderAdmissionAdvertError, ProviderAdmissionEnvelopeError, ProviderAdmissionEnvelopeV1,
    ProviderAdmissionProposalV1, ProviderAdmissionRenewalError, ProviderAdmissionRenewalV1,
    ProviderAdmissionRevocationError, ProviderAdmissionRevocationV1,
    ProviderAdmissionSignatureError, ProviderAdmissionValidationError, compute_advert_body_digest,
    compute_envelope_digest, compute_proposal_digest, verify_advert_against_record,
    verify_envelope, verify_revocation_signatures,
};
pub use provider_advert::{
    AdvertEndpoint, AdvertSignature, AdvertValidationError, AvailabilityTier, CapabilityTlv,
    CapabilityType, EndpointKind, EndpointMetadata, EndpointMetadataKey, MAX_ADVERT_TTL_SECS,
    PROVIDER_ADVERT_VERSION_V1, PathDiversityPolicy, ProviderAdvertBodyV1,
    ProviderAdvertBuildError, ProviderAdvertBuilder, ProviderAdvertV1, ProviderCapabilityRangeV1,
    QosHints, REFRESH_RECOMMENDATION_SECS, RangeCapabilityError, RendezvousTopic,
    SignatureAlgorithm, StakePointer, StreamBudgetError, StreamBudgetV1, TransportHintError,
    TransportHintV1, TransportProtocol,
};
pub use repair::{
    REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, REPAIR_SLASH_PROPOSAL_VERSION_V1,
    REPAIR_TASK_VERSION_V1, RepairCauseV1, RepairEvidenceV1, RepairReportV1, RepairSlashProposalV1,
    RepairTaskRecordV1, RepairTaskStateV1, RepairTicketId, RepairValidationError,
};
pub use token::{StreamTokenBodyV1, StreamTokenError, StreamTokenV1};
pub use validation::{
    ManifestValidationError, PinPolicyConstraints, validate_chunker_handle, validate_manifest,
    validate_pin_policy,
};

pub use self::gateway_fixture::{
    GatewayFixtureMetadata, SORAFS_GATEWAY_CAR_DIGEST_HEX, SORAFS_GATEWAY_FIXTURE_DIGEST_HEX,
    SORAFS_GATEWAY_FIXTURE_RELEASE_UNIX, SORAFS_GATEWAY_FIXTURE_VERSION,
    SORAFS_GATEWAY_MANIFEST_DIGEST_HEX, SORAFS_GATEWAY_PAYLOAD_DIGEST_HEX,
    SORAFS_GATEWAY_PROFILE_VERSION, gateway_fixture_digest_hex, gateway_fixture_metadata,
};

/// Manifest version identifier.
pub const MANIFEST_VERSION_V1: u8 = 1;

/// Multihash code for BLAKE3-256.
pub const BLAKE3_256_MULTIHASH_CODE: u64 = 0x1f;

/// Norito-encoded manifest (version 1).
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ManifestV1 {
    pub version: u8,
    pub root_cid: Vec<u8>,
    pub dag_codec: DagCodecId,
    pub chunking: ChunkingProfileV1,
    pub content_length: u64,
    pub car_digest: [u8; 32],
    pub car_size: u64,
    pub pin_policy: PinPolicy,
    pub governance: GovernanceProofs,
    pub alias_claims: Vec<AliasClaim>,
    pub metadata: Vec<MetadataEntry>,
}

impl ManifestV1 {
    /// Serializes the manifest using canonical Norito encoding.
    pub fn encode(&self) -> Result<Vec<u8>, NoritoError> {
        norito::to_bytes(self)
    }

    /// Computes the canonical manifest digest used by the Pin Registry.
    pub fn digest(&self) -> Result<Hash, NoritoError> {
        let bytes = self.encode()?;
        Ok(blake3::hash(&bytes))
    }
}

/// Errors raised while building a manifest.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ManifestBuildError {
    #[error("missing required field: {0}")]
    MissingField(&'static str),
}

/// Builder for [`ManifestV1`].
#[derive(Debug, Default)]
pub struct ManifestBuilder {
    root_cid: Option<Vec<u8>>,
    dag_codec: Option<DagCodecId>,
    chunking: Option<ChunkingProfileV1>,
    content_length: Option<u64>,
    car_digest: Option<[u8; 32]>,
    car_size: Option<u64>,
    pin_policy: Option<PinPolicy>,
    governance: GovernanceProofs,
    alias_claims: Vec<AliasClaim>,
    metadata: Vec<MetadataEntry>,
}

impl ManifestBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn root_cid(mut self, cid: impl Into<Vec<u8>>) -> Self {
        self.root_cid = Some(cid.into());
        self
    }

    #[must_use]
    pub fn dag_codec(mut self, codec: DagCodecId) -> Self {
        self.dag_codec = Some(codec);
        self
    }

    #[must_use]
    pub fn chunking_profile(mut self, profile: ChunkingProfileV1) -> Self {
        self.chunking = Some(profile);
        self
    }

    #[must_use]
    pub fn chunking_from_registry(mut self, profile_id: ProfileId) -> Self {
        if let Some(descriptor) = crate::chunker_registry::lookup(profile_id) {
            self.chunking = Some(ChunkingProfileV1::from_descriptor(descriptor));
        }
        self
    }

    #[must_use]
    pub fn chunking_from_profile(mut self, profile: ChunkProfile, multihash_code: u64) -> Self {
        self.chunking = Some(ChunkingProfileV1::from_profile(profile, multihash_code));
        self
    }

    #[must_use]
    pub fn content_length(mut self, len: u64) -> Self {
        self.content_length = Some(len);
        self
    }

    #[must_use]
    pub fn car_digest(mut self, digest: [u8; 32]) -> Self {
        self.car_digest = Some(digest);
        self
    }

    #[must_use]
    pub fn car_size(mut self, size: u64) -> Self {
        self.car_size = Some(size);
        self
    }

    #[must_use]
    pub fn pin_policy(mut self, policy: PinPolicy) -> Self {
        self.pin_policy = Some(policy);
        self
    }

    #[must_use]
    pub fn governance(mut self, proofs: GovernanceProofs) -> Self {
        self.governance = proofs;
        self
    }

    #[must_use]
    pub fn push_alias(mut self, claim: AliasClaim) -> Self {
        self.alias_claims.push(claim);
        self
    }

    #[must_use]
    pub fn extend_aliases<I>(mut self, claims: I) -> Self
    where
        I: IntoIterator<Item = AliasClaim>,
    {
        self.alias_claims.extend(claims);
        self
    }

    #[must_use]
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.push(MetadataEntry {
            key: key.into(),
            value: value.into(),
        });
        self
    }

    #[must_use]
    pub fn extend_metadata<I>(mut self, entries: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        self.metadata.extend(
            entries
                .into_iter()
                .map(|(key, value)| MetadataEntry { key, value }),
        );
        self
    }

    pub fn build(self) -> Result<ManifestV1, ManifestBuildError> {
        let root_cid = self
            .root_cid
            .ok_or(ManifestBuildError::MissingField("root_cid"))?;
        let dag_codec = self
            .dag_codec
            .ok_or(ManifestBuildError::MissingField("dag_codec"))?;
        let chunking = self
            .chunking
            .ok_or(ManifestBuildError::MissingField("chunking"))?;
        let content_length = self
            .content_length
            .ok_or(ManifestBuildError::MissingField("content_length"))?;
        let car_digest = self
            .car_digest
            .ok_or(ManifestBuildError::MissingField("car_digest"))?;
        let car_size = self
            .car_size
            .ok_or(ManifestBuildError::MissingField("car_size"))?;
        let pin_policy = self
            .pin_policy
            .ok_or(ManifestBuildError::MissingField("pin_policy"))?;

        Ok(ManifestV1 {
            version: MANIFEST_VERSION_V1,
            root_cid,
            dag_codec,
            chunking,
            content_length,
            car_digest,
            car_size,
            pin_policy,
            governance: self.governance,
            alias_claims: self.alias_claims,
            metadata: self.metadata,
        })
    }
}

/// Simple newtype for Dag codec identifiers (CID multicodec).
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
pub struct DagCodecId(pub u64);

/// Snapshot of the chunking profile baked into the manifest.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ChunkingProfileV1 {
    pub profile_id: ProfileId,
    pub namespace: String,
    pub name: String,
    pub semver: String,
    pub min_size: u32,
    pub target_size: u32,
    pub max_size: u32,
    pub break_mask: u32,
    pub multihash_code: u64,
    pub aliases: Vec<String>,
}

impl ChunkingProfileV1 {
    pub fn from_profile(profile: ChunkProfile, multihash_code: u64) -> Self {
        if let Some(descriptor) =
            crate::chunker_registry::lookup_by_profile(profile, multihash_code)
        {
            Self::from_descriptor(descriptor)
        } else {
            Self {
                profile_id: ProfileId(0),
                namespace: "inline".to_owned(),
                name: "inline".to_owned(),
                semver: "0.0.0".to_owned(),
                min_size: profile.min_size as u32,
                target_size: profile.target_size as u32,
                max_size: profile.max_size as u32,
                break_mask: profile.break_mask as u32,
                multihash_code,
                aliases: vec!["inline.inline@0.0.0".to_owned()],
            }
        }
    }

    pub fn from_descriptor(descriptor: &crate::chunker_registry::ChunkerProfileDescriptor) -> Self {
        Self {
            profile_id: descriptor.id,
            namespace: descriptor.namespace.to_owned(),
            name: descriptor.name.to_owned(),
            semver: descriptor.semver.to_owned(),
            min_size: descriptor.profile.min_size as u32,
            target_size: descriptor.profile.target_size as u32,
            max_size: descriptor.profile.max_size as u32,
            break_mask: descriptor.profile.break_mask as u32,
            multihash_code: descriptor.multihash_code,
            aliases: descriptor
                .aliases
                .iter()
                .map(|alias| alias.to_string())
                .collect(),
        }
    }
}

/// Profile identifier used for chunking negotiation.
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
    PartialOrd,
    Ord,
)]
pub struct ProfileId(pub u32);

/// Storage replication policy encoded in the manifest.
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
pub struct PinPolicy {
    pub min_replicas: u16,
    pub storage_class: StorageClass,
    pub retention_epoch: u64,
}

impl Default for PinPolicy {
    fn default() -> Self {
        Self {
            min_replicas: 1,
            storage_class: StorageClass::default(),
            retention_epoch: 0,
        }
    }
}

/// Storage tier expressed in the manifest.
#[derive(
    Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq, PartialOrd, Ord, Default,
)]
pub enum StorageClass {
    #[default]
    Hot,
    Warm,
    Cold,
}

impl FastJsonWrite for StorageClass {
    fn write_json(&self, out: &mut String) {
        let label = match self {
            StorageClass::Hot => "hot",
            StorageClass::Warm => "warm",
            StorageClass::Cold => "cold",
        };
        NoritoJsonSerialize::json_serialize(&label, out);
    }
}

impl json::JsonDeserialize for StorageClass {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "hot" => Ok(StorageClass::Hot),
            "warm" => Ok(StorageClass::Warm),
            "cold" => Ok(StorageClass::Cold),
            other => Err(json::Error::Message(format!(
                "unknown storage class `{other}`"
            ))),
        }
    }
}

/// Governance proof bundle.
///
/// Future policy proofs (e.g., admission allowlists, replication attestations)
/// will be threaded through this container once the registry schema lands.
#[derive(
    Debug,
    Clone,
    Default,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct GovernanceProofs {
    pub council_signatures: Vec<CouncilSignature>,
}

/// Council signature proof binding the manifest digest.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct CouncilSignature {
    pub signer: [u8; 32],
    pub signature: Vec<u8>,
}

/// Alias binding bundled with the manifest.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct AliasClaim {
    pub name: String,
    pub namespace: String,
    pub proof: Vec<u8>,
}

/// Metadata key/value pair recorded in the manifest.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct MetadataEntry {
    pub key: String,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> ManifestV1 {
        ManifestBuilder::new()
            .root_cid(vec![0x01, 0x55, 0xaa])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(ChunkProfile::DEFAULT, BLAKE3_256_MULTIHASH_CODE)
            .content_length(1_048_576)
            .car_digest([0xAB; 32])
            .car_size(1_100_000)
            .pin_policy(PinPolicy {
                min_replicas: 3,
                storage_class: StorageClass::Hot,
                retention_epoch: 42,
            })
            .governance(GovernanceProofs {
                council_signatures: vec![CouncilSignature {
                    signer: [0x11; 32],
                    signature: vec![0x22; 64],
                }],
            })
            .push_alias(AliasClaim {
                name: "docs".into(),
                namespace: "sora".into(),
                proof: vec![0xde, 0xad, 0xbe, 0xef],
            })
            .add_metadata("build", "ci-123")
            .add_metadata("commit", "abc123")
            .build()
            .expect("build manifest")
    }

    #[test]
    fn encode_roundtrip() {
        let manifest = sample_manifest();
        let bytes = manifest.encode().expect("encode manifest");
        let decoded: ManifestV1 = norito::decode_from_bytes(&bytes).expect("decode manifest");
        assert_eq!(manifest, decoded);
    }

    #[test]
    fn digest_is_deterministic() {
        let manifest = sample_manifest();
        let digest_a = manifest.digest().expect("digest");
        let digest_b = manifest.digest().expect("digest again");
        assert_eq!(digest_a.as_bytes(), digest_b.as_bytes());
        assert_eq!(manifest.chunking.namespace, "sorafs");
        assert_eq!(manifest.chunking.name, "sf1");
        assert_eq!(manifest.chunking.semver, "1.0.0");
    }

    #[test]
    fn builder_rejects_missing_fields() {
        let err = ManifestBuilder::new().build().unwrap_err();
        assert!(matches!(err, ManifestBuildError::MissingField("root_cid")));
    }

    #[test]
    fn chunking_profile_includes_registry_aliases() {
        let manifest = sample_manifest();
        let descriptor = crate::chunker_registry::lookup(manifest.chunking.profile_id)
            .expect("descriptor for registered profile");
        let expected: Vec<String> = descriptor
            .aliases
            .iter()
            .map(|alias| alias.to_string())
            .collect();
        assert_eq!(manifest.chunking.aliases, expected);
    }

    #[test]
    fn chunking_profile_fallback_has_inline_alias() {
        let custom_profile = ChunkProfile {
            min_size: 128,
            target_size: 256,
            max_size: 512,
            break_mask: 0xff,
        };
        let chunking = ChunkingProfileV1::from_profile(custom_profile, 0xdead_beef);
        assert_eq!(chunking.aliases, vec!["inline.inline@0.0.0".to_owned()]);
    }
}
