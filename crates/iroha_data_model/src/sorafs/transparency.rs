//! Transparency ledger schemas and deterministic proof helpers for SoraFS.
//!
//! The SFM-4c transparency service publishes privacy-safe summaries of
//! moderation, appeal, GAR, proof-token, and evidence-access activity. This
//! module defines the canonical V1 entry, block, and inclusion-proof payloads
//! plus the deterministic BLAKE3 Merkle helpers needed by publishers and public
//! verifiers.

use std::collections::{BTreeMap, BTreeSet};

use blake3::Hasher;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

/// Schema version for [`ModerationLedgerEntryV1`].
pub const MODERATION_LEDGER_ENTRY_VERSION_V1: u16 = 1;
/// Schema version for [`ModerationLedgerBlockV1`].
pub const MODERATION_LEDGER_BLOCK_VERSION_V1: u16 = 1;
/// Schema version for [`ModerationLedgerProofV1`].
pub const MODERATION_LEDGER_PROOF_VERSION_V1: u16 = 1;
/// Schema version for [`ModerationLedgerCyclePublicationV1`].
pub const MODERATION_LEDGER_PUBLICATION_VERSION_V1: u16 = 1;
/// Schema version for [`ModerationPrivacyAggregateV1`].
pub const MODERATION_PRIVACY_AGGREGATE_VERSION_V1: u16 = 1;
/// Schema version for [`ModerationPrivacyParametersV1`].
pub const MODERATION_PRIVACY_PARAMETERS_VERSION_V1: u16 = 1;
/// Schema version for [`ProofTokenIssuanceV1`].
pub const PROOF_TOKEN_ISSUANCE_VERSION_V1: u16 = 1;
/// Maximum Merkle audit-path length accepted by the transparency verifier.
pub const MODERATION_LEDGER_MAX_PROOF_PATH_LEN: usize = 64;
/// Maximum encoded `delta_ppb` value, equal to probability 1.0.
pub const MODERATION_PRIVACY_DELTA_PPB_MAX: u64 = 1_000_000_000;

const ENTRY_HASH_DOMAIN_V1: &[u8] = b"sorafs.transparency.entry.v1";
const BLOCK_HASH_DOMAIN_V1: &[u8] = b"sorafs.transparency.block.v1";
const PUBLICATION_HASH_DOMAIN_V1: &[u8] = b"sorafs.transparency.publication.v1";
const MERKLE_NODE_DOMAIN_V1: &[u8] = b"sorafs.transparency.node.v1";
const PRIVACY_AGGREGATE_HASH_DOMAIN_V1: &[u8] = b"sorafs.transparency.privacy_aggregate.v1";
const PRIVACY_AGGREGATE_SUBJECT_DOMAIN_V1: &[u8] =
    b"sorafs.transparency.privacy_aggregate.subject.v1";
const PROOF_TOKEN_ISSUANCE_HASH_DOMAIN_V1: &[u8] = b"sorafs.transparency.proof_token_issuance.v1";
const PROOF_TOKEN_SUBJECT_DOMAIN_V1: &[u8] = b"sorafs.transparency.proof_token.subject.v1";

/// Transparency ledger entry kind.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum ModerationLedgerEntryKindV1 {
    /// Moderation action summary.
    ModerationAction,
    /// Appeal outcome and deposit disposition summary.
    AppealOutcome,
    /// GAR enforcement receipt summary.
    GarEnforcementReceipt,
    /// Proof-token issuance or revocation summary.
    ProofTokenIssuance,
    /// Evidence-viewer access audit summary.
    EvidenceAccess,
    /// Differential-privacy or suppression aggregate summary.
    PrivacyAggregate,
    /// Legal hold or guardian freeze summary.
    LegalHold,
    /// Redaction summary for a previously public entry.
    Redaction,
    /// Domain-specific entry kind identified by a governance-controlled slug.
    Custom(String),
}

/// Public metadata attached to a transparency ledger entry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationLedgerMetadataV1 {
    /// Metadata key.
    pub key: String,
    /// Metadata value.
    pub value: String,
}

/// Privacy mode used for an aggregate row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(tag = "mode", content = "value", rename_all = "snake_case")
)]
pub enum ModerationPrivacyModeV1 {
    /// Differential privacy parameters are present.
    DifferentialPrivacy,
    /// Small groups are suppressed without adding noise.
    Suppression,
    /// Differential privacy and suppression parameters are both present.
    DifferentialPrivacyWithSuppression,
}

/// Explicit privacy parameters for a transparency aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationPrivacyParametersV1 {
    /// Schema version; must equal [`MODERATION_PRIVACY_PARAMETERS_VERSION_V1`].
    pub version: u16,
    /// Privacy mode applied to the metrics.
    pub mode: ModerationPrivacyModeV1,
    /// Epsilon encoded as fixed-point micros for deterministic publication.
    #[norito(default)]
    pub epsilon_micros: Option<u64>,
    /// Delta encoded in parts per billion for deterministic publication.
    #[norito(default)]
    pub delta_ppb: Option<u64>,
    /// Optional noise scale encoded as fixed-point micros.
    #[norito(default)]
    pub noise_scale_micros: Option<u64>,
    /// Minimum source-event count required before a bucket can be published.
    #[norito(default)]
    pub suppression_threshold: Option<u64>,
    /// Number of source buckets suppressed before publication.
    pub suppressed_count: u64,
}

/// One privacy-safe aggregate metric.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationPrivacyAggregateMetricV1 {
    /// Stable metric key.
    pub key: String,
    /// Published privacy-safe value.
    pub value: u64,
    /// Unit label for the published value.
    pub unit: String,
}

/// Canonical privacy-safe moderation aggregate for SFM-4c dashboards.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationPrivacyAggregateV1 {
    /// Schema version; must equal [`MODERATION_PRIVACY_AGGREGATE_VERSION_V1`].
    pub version: u16,
    /// Stable aggregate identifier safe to disclose.
    pub aggregate_id: String,
    /// Inclusive source-event window start timestamp, in Unix seconds.
    pub window_start_unix: u64,
    /// Exclusive source-event window end timestamp, in Unix seconds.
    pub window_end_unix: u64,
    /// Unix timestamp (seconds) when the aggregate was generated.
    pub generated_at_unix: u64,
    /// Privacy-safe population label, such as a jurisdiction or policy bucket.
    pub population_label: String,
    /// BLAKE3 digest of the canonical private population selector.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub population_digest: [u8; 32],
    /// Explicit privacy parameters applied before publication.
    pub privacy: ModerationPrivacyParametersV1,
    /// Number of source events considered before privacy filtering/noising.
    pub source_event_count: u64,
    /// BLAKE3 digest of the canonical source aggregate payload.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub source_payload_digest: [u8; 32],
    /// Published privacy-safe metrics, sorted by key.
    pub metrics: Vec<ModerationPrivacyAggregateMetricV1>,
    /// Optional digest of the aggregate policy/configuration.
    #[norito(default)]
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub policy_digest: Option<[u8; 32]>,
    /// Public key/value metadata. Keys must be unique and sorted.
    #[norito(default)]
    pub metadata: Vec<ModerationLedgerMetadataV1>,
}

/// Canonical privacy-safe record for one issued `SoraFS` moderation proof token.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ProofTokenIssuanceV1 {
    /// Schema version; must equal [`PROOF_TOKEN_ISSUANCE_VERSION_V1`].
    pub version: u16,
    /// Opaque proof-token identifier from the `SFGT` frame.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub token_id: [u8; 16],
    /// Unix timestamp (seconds) when the token becomes valid.
    pub issued_at_unix: u64,
    /// Optional token expiry timestamp, in Unix seconds.
    #[norito(default)]
    pub expires_at_unix: Option<u64>,
    /// Moderation action code embedded in the token body.
    pub moderation_action_code: u8,
    /// Ed25519 verifying key of the gateway proof-token signer.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub signer_key: [u8; 32],
    /// BLAKE3 digest of the raw encoded proof-token frame.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub token_blake3: [u8; 32],
    /// Token blinded digest. This is public verification material, not the digest key.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub blinded_digest: [u8; 32],
    /// Public denylist or moderation-entry identifiers bound to the token.
    pub entry_ids: Vec<String>,
    /// Optional digest of the runtime evidence bundle.
    #[norito(default)]
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub evidence_digest: Option<[u8; 32]>,
    /// Optional digest of the policy/configuration that governed issuance.
    #[norito(default)]
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub policy_digest: Option<[u8; 32]>,
    /// Public key/value metadata. Keys must be unique and sorted.
    #[norito(default)]
    pub metadata: Vec<ModerationLedgerMetadataV1>,
}

/// Canonical SFM-4c transparency ledger entry.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationLedgerEntryV1 {
    /// Schema version; must equal [`MODERATION_LEDGER_ENTRY_VERSION_V1`].
    pub version: u16,
    /// Cycle identifier this entry belongs to.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub cycle_id: [u8; 16],
    /// Unique entry identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub entry_id: [u8; 16],
    /// Monotonic publisher sequence within the cycle.
    pub sequence: u64,
    /// Unix timestamp (seconds) when the source event occurred.
    pub occurred_at_unix: u64,
    /// Entry category.
    pub kind: ModerationLedgerEntryKindV1,
    /// Privacy-safe subject label, such as a case id, GAR host, or aggregate id.
    pub subject: String,
    /// BLAKE3 digest of the canonical subject identifier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub subject_digest: [u8; 32],
    /// BLAKE3 digest of the canonical source payload represented by this entry.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub payload_digest: [u8; 32],
    /// BLAKE3 digest of the public summary emitted for dashboards/explorers.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub summary_digest: [u8; 32],
    /// Optional digest of the policy/configuration that governed this entry.
    #[norito(default)]
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub policy_digest: Option<[u8; 32]>,
    /// Optional public evidence URIs safe to disclose.
    #[norito(default)]
    pub evidence_uris: Vec<String>,
    /// Public key/value metadata. Keys must be unique and sorted.
    #[norito(default)]
    pub metadata: Vec<ModerationLedgerMetadataV1>,
}

/// Canonical transparency ledger cycle header.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationLedgerBlockV1 {
    /// Schema version; must equal [`MODERATION_LEDGER_BLOCK_VERSION_V1`].
    pub version: u16,
    /// Cycle identifier shared by all entries covered by this block.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub cycle_id: [u8; 16],
    /// Inclusive cycle start timestamp, in Unix seconds.
    pub cycle_start_unix: u64,
    /// Exclusive cycle end timestamp, in Unix seconds.
    pub cycle_end_unix: u64,
    /// Unix timestamp (seconds) when the block was generated.
    pub generated_at_unix: u64,
    /// Number of entries covered by [`Self::entry_root`].
    pub entry_count: u32,
    /// Merkle root over sorted entry hashes.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub entry_root: [u8; 32],
    /// Optional hash of the previous transparency ledger block.
    #[norito(default)]
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::option")
    )]
    pub previous_block_hash: Option<[u8; 32]>,
}

/// Relative position of a sibling in a Merkle audit path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(tag = "side", content = "value", rename_all = "snake_case")
)]
pub enum ModerationLedgerProofSideV1 {
    /// Sibling hash is left of the current node.
    Left,
    /// Sibling hash is right of the current node.
    Right,
}

/// One sibling node in a transparency ledger Merkle proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationLedgerProofNodeV1 {
    /// Sibling position relative to the current node.
    pub side: ModerationLedgerProofSideV1,
    /// Sibling hash.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub hash: [u8; 32],
}

/// Inclusion proof for a transparency ledger entry.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationLedgerProofV1 {
    /// Schema version; must equal [`MODERATION_LEDGER_PROOF_VERSION_V1`].
    pub version: u16,
    /// Cycle identifier the proof is bound to.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub cycle_id: [u8; 16],
    /// Index of the entry in the sorted Merkle leaf set.
    pub leaf_index: u32,
    /// Entry included by the proof.
    pub entry: ModerationLedgerEntryV1,
    /// Canonical hash of [`Self::entry`].
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub entry_hash: [u8; 32],
    /// Merkle root this proof claims.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub root: [u8; 32],
    /// Leaf-to-root sibling path.
    #[norito(default)]
    pub audit_path: Vec<ModerationLedgerProofNodeV1>,
}

/// Canonical SFM-4c cycle publication bundle.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ModerationLedgerCyclePublicationV1 {
    /// Schema version; must equal [`MODERATION_LEDGER_PUBLICATION_VERSION_V1`].
    pub version: u16,
    /// Cycle header being published.
    pub block: ModerationLedgerBlockV1,
    /// Inclusion proofs for every entry covered by [`Self::block`], sorted by leaf index.
    pub proofs: Vec<ModerationLedgerProofV1>,
}

impl ModerationPrivacyParametersV1 {
    /// Validate the explicit privacy parameters for an aggregate.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError`] when the version is unsupported or
    /// the selected mode is missing required epsilon/delta/suppression fields.
    pub fn validate(&self) -> Result<(), TransparencyLedgerError> {
        if self.version != MODERATION_PRIVACY_PARAMETERS_VERSION_V1 {
            return Err(
                TransparencyLedgerError::UnsupportedPrivacyParametersVersion {
                    expected: MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
                    found: self.version,
                },
            );
        }
        match self.mode {
            ModerationPrivacyModeV1::DifferentialPrivacy => {
                require_positive_parameter("epsilon_micros", self.epsilon_micros)?;
                require_delta_parameter(self.delta_ppb)?;
                require_positive_optional_parameter("noise_scale_micros", self.noise_scale_micros)?;
                require_absent_parameter("suppression_threshold", self.suppression_threshold)?;
                if self.suppressed_count != 0 {
                    return Err(TransparencyLedgerError::InvalidPrivacyParameter {
                        field: "suppressed_count",
                    });
                }
            }
            ModerationPrivacyModeV1::Suppression => {
                require_absent_parameter("epsilon_micros", self.epsilon_micros)?;
                require_absent_parameter("delta_ppb", self.delta_ppb)?;
                require_absent_parameter("noise_scale_micros", self.noise_scale_micros)?;
                require_positive_parameter("suppression_threshold", self.suppression_threshold)?;
            }
            ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression => {
                require_positive_parameter("epsilon_micros", self.epsilon_micros)?;
                require_delta_parameter(self.delta_ppb)?;
                require_positive_optional_parameter("noise_scale_micros", self.noise_scale_micros)?;
                require_positive_parameter("suppression_threshold", self.suppression_threshold)?;
            }
        }
        Ok(())
    }
}

impl ModerationPrivacyAggregateV1 {
    /// Validate this privacy-safe aggregate payload.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError`] when versioning, timestamps,
    /// digests, privacy parameters, metrics, or public metadata are malformed.
    pub fn validate(&self) -> Result<(), TransparencyLedgerError> {
        if self.version != MODERATION_PRIVACY_AGGREGATE_VERSION_V1 {
            return Err(
                TransparencyLedgerError::UnsupportedPrivacyAggregateVersion {
                    expected: MODERATION_PRIVACY_AGGREGATE_VERSION_V1,
                    found: self.version,
                },
            );
        }
        require_public_text("aggregate_id", &self.aggregate_id)?;
        if self.window_start_unix == 0 {
            return Err(TransparencyLedgerError::InvalidTimestamp {
                field: "window_start_unix",
            });
        }
        if self.window_end_unix <= self.window_start_unix {
            return Err(TransparencyLedgerError::InvalidPrivacyAggregateWindow);
        }
        if self.generated_at_unix < self.window_end_unix {
            return Err(TransparencyLedgerError::InvalidPrivacyAggregateGeneratedAt);
        }
        require_public_text("population_label", &self.population_label)?;
        require_nonzero32("population_digest", &self.population_digest)?;
        self.privacy.validate()?;
        if self.source_event_count == 0 {
            return Err(TransparencyLedgerError::InvalidPrivacyParameter {
                field: "source_event_count",
            });
        }
        require_nonzero32("source_payload_digest", &self.source_payload_digest)?;
        validate_privacy_metrics(&self.metrics)?;
        if let Some(policy_digest) = &self.policy_digest {
            require_nonzero32("policy_digest", policy_digest)?;
        }
        validate_metadata(&self.metadata)?;
        Ok(())
    }

    /// Compute the domain-separated canonical aggregate hash.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError::CanonicalEncode`] if Norito encoding fails.
    pub fn aggregate_hash(&self) -> Result<[u8; 32], TransparencyLedgerError> {
        hash_norito(PRIVACY_AGGREGATE_HASH_DOMAIN_V1, self)
    }

    /// Convert this aggregate payload into a public transparency ledger entry.
    ///
    /// The resulting entry uses [`ModerationLedgerEntryKindV1::PrivacyAggregate`],
    /// stores the canonical aggregate hash as both payload and summary digest,
    /// and includes privacy parameters as sorted public metadata.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError`] when the aggregate or target entry
    /// identifiers are malformed, or when aggregate metadata collides with the
    /// reserved ledger metadata keys emitted by this conversion.
    pub fn to_ledger_entry(
        &self,
        cycle_id: [u8; 16],
        entry_id: [u8; 16],
        sequence: u64,
    ) -> Result<ModerationLedgerEntryV1, TransparencyLedgerError> {
        self.validate()?;
        require_nonzero16("cycle_id", &cycle_id)?;
        require_nonzero16("entry_id", &entry_id)?;

        let payload_digest = self.aggregate_hash()?;
        let mut metadata = BTreeMap::new();
        insert_metadata(&mut metadata, "aggregate_id", self.aggregate_id.clone())?;
        if let Some(delta_ppb) = self.privacy.delta_ppb {
            insert_metadata(&mut metadata, "delta_ppb", delta_ppb.to_string())?;
        }
        if let Some(epsilon_micros) = self.privacy.epsilon_micros {
            insert_metadata(&mut metadata, "epsilon_micros", epsilon_micros.to_string())?;
        }
        insert_metadata(
            &mut metadata,
            "population_label",
            self.population_label.clone(),
        )?;
        insert_metadata(
            &mut metadata,
            "privacy_mode",
            privacy_mode_label(self.privacy.mode).to_string(),
        )?;
        if let Some(noise_scale_micros) = self.privacy.noise_scale_micros {
            insert_metadata(
                &mut metadata,
                "noise_scale_micros",
                noise_scale_micros.to_string(),
            )?;
        }
        insert_metadata(
            &mut metadata,
            "source_event_count",
            self.source_event_count.to_string(),
        )?;
        if let Some(suppression_threshold) = self.privacy.suppression_threshold {
            insert_metadata(
                &mut metadata,
                "suppression_threshold",
                suppression_threshold.to_string(),
            )?;
        }
        insert_metadata(
            &mut metadata,
            "suppressed_count",
            self.privacy.suppressed_count.to_string(),
        )?;
        insert_metadata(
            &mut metadata,
            "window_end_unix",
            self.window_end_unix.to_string(),
        )?;
        insert_metadata(
            &mut metadata,
            "window_start_unix",
            self.window_start_unix.to_string(),
        )?;
        for item in &self.metadata {
            insert_metadata(&mut metadata, &item.key, item.value.clone())?;
        }

        let entry = ModerationLedgerEntryV1 {
            version: MODERATION_LEDGER_ENTRY_VERSION_V1,
            cycle_id,
            entry_id,
            sequence,
            occurred_at_unix: self.generated_at_unix,
            kind: ModerationLedgerEntryKindV1::PrivacyAggregate,
            subject: self.aggregate_id.clone(),
            subject_digest: hash_text(PRIVACY_AGGREGATE_SUBJECT_DOMAIN_V1, &self.aggregate_id),
            payload_digest,
            summary_digest: payload_digest,
            policy_digest: self.policy_digest,
            evidence_uris: Vec::new(),
            metadata: metadata
                .into_iter()
                .map(|(key, value)| ModerationLedgerMetadataV1 { key, value })
                .collect(),
        };
        entry.validate()?;
        Ok(entry)
    }
}

impl ProofTokenIssuanceV1 {
    /// Validate this proof-token issuance record.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError`] when versioning, timestamps,
    /// digests, entry identifiers, or public metadata are malformed.
    pub fn validate(&self) -> Result<(), TransparencyLedgerError> {
        if self.version != PROOF_TOKEN_ISSUANCE_VERSION_V1 {
            return Err(
                TransparencyLedgerError::UnsupportedProofTokenIssuanceVersion {
                    expected: PROOF_TOKEN_ISSUANCE_VERSION_V1,
                    found: self.version,
                },
            );
        }
        require_nonzero16("token_id", &self.token_id)?;
        if self.issued_at_unix == 0 {
            return Err(TransparencyLedgerError::InvalidTimestamp {
                field: "issued_at_unix",
            });
        }
        if let Some(expires_at_unix) = self.expires_at_unix
            && expires_at_unix <= self.issued_at_unix
        {
            return Err(TransparencyLedgerError::InvalidProofTokenExpiry);
        }
        require_nonzero32("signer_key", &self.signer_key)?;
        require_nonzero32("token_blake3", &self.token_blake3)?;
        require_nonzero32("blinded_digest", &self.blinded_digest)?;
        validate_entry_ids(&self.entry_ids)?;
        if let Some(evidence_digest) = &self.evidence_digest {
            require_nonzero32("evidence_digest", evidence_digest)?;
        }
        if let Some(policy_digest) = &self.policy_digest {
            require_nonzero32("policy_digest", policy_digest)?;
        }
        validate_metadata(&self.metadata)?;
        Ok(())
    }

    /// Compute the domain-separated canonical issuance hash.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError::CanonicalEncode`] if Norito encoding fails.
    pub fn issuance_hash(&self) -> Result<[u8; 32], TransparencyLedgerError> {
        hash_norito(PROOF_TOKEN_ISSUANCE_HASH_DOMAIN_V1, self)
    }

    /// Convert this proof-token issuance record into a public transparency ledger entry.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError`] when the issuance record or target
    /// ledger identifiers are malformed, or when caller metadata collides with
    /// reserved proof-token metadata keys.
    pub fn to_ledger_entry(
        &self,
        cycle_id: [u8; 16],
        entry_id: [u8; 16],
        sequence: u64,
    ) -> Result<ModerationLedgerEntryV1, TransparencyLedgerError> {
        self.validate()?;
        require_nonzero16("cycle_id", &cycle_id)?;
        require_nonzero16("entry_id", &entry_id)?;

        let payload_digest = self.issuance_hash()?;
        let token_id_hex = hex::encode(self.token_id);
        let mut metadata = BTreeMap::new();
        insert_metadata(
            &mut metadata,
            "blinded_digest_hex",
            hex::encode(self.blinded_digest),
        )?;
        insert_metadata(
            &mut metadata,
            "entry_count",
            self.entry_ids.len().to_string(),
        )?;
        if let Some(expires_at_unix) = self.expires_at_unix {
            insert_metadata(
                &mut metadata,
                "expires_at_unix",
                expires_at_unix.to_string(),
            )?;
        }
        insert_metadata(
            &mut metadata,
            "issued_at_unix",
            self.issued_at_unix.to_string(),
        )?;
        insert_metadata(
            &mut metadata,
            "moderation_action_code",
            self.moderation_action_code.to_string(),
        )?;
        insert_metadata(
            &mut metadata,
            "signer_key_hex",
            hex::encode(self.signer_key),
        )?;
        insert_metadata(
            &mut metadata,
            "token_blake3_hex",
            hex::encode(self.token_blake3),
        )?;
        insert_metadata(&mut metadata, "token_id_hex", token_id_hex.clone())?;
        for item in &self.metadata {
            insert_metadata(&mut metadata, &item.key, item.value.clone())?;
        }

        let entry = ModerationLedgerEntryV1 {
            version: MODERATION_LEDGER_ENTRY_VERSION_V1,
            cycle_id,
            entry_id,
            sequence,
            occurred_at_unix: self.issued_at_unix,
            kind: ModerationLedgerEntryKindV1::ProofTokenIssuance,
            subject: format!("proof-token:{token_id_hex}"),
            subject_digest: hash_text(PROOF_TOKEN_SUBJECT_DOMAIN_V1, &token_id_hex),
            payload_digest,
            summary_digest: payload_digest,
            policy_digest: self.policy_digest,
            evidence_uris: Vec::new(),
            metadata: metadata
                .into_iter()
                .map(|(key, value)| ModerationLedgerMetadataV1 { key, value })
                .collect(),
        };
        entry.validate()?;
        Ok(entry)
    }
}

impl ModerationLedgerEntryV1 {
    /// Validate this ledger entry.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError`] when versioning, identifiers,
    /// timestamps, digests, metadata, or public text fields are malformed.
    pub fn validate(&self) -> Result<(), TransparencyLedgerError> {
        if self.version != MODERATION_LEDGER_ENTRY_VERSION_V1 {
            return Err(TransparencyLedgerError::UnsupportedEntryVersion {
                expected: MODERATION_LEDGER_ENTRY_VERSION_V1,
                found: self.version,
            });
        }
        require_nonzero16("cycle_id", &self.cycle_id)?;
        require_nonzero16("entry_id", &self.entry_id)?;
        if self.occurred_at_unix == 0 {
            return Err(TransparencyLedgerError::InvalidTimestamp {
                field: "occurred_at_unix",
            });
        }
        require_public_text("subject", &self.subject)?;
        require_nonzero32("subject_digest", &self.subject_digest)?;
        require_nonzero32("payload_digest", &self.payload_digest)?;
        require_nonzero32("summary_digest", &self.summary_digest)?;
        if let Some(policy_digest) = &self.policy_digest {
            require_nonzero32("policy_digest", policy_digest)?;
        }
        validate_metadata(&self.metadata)?;
        for uri in &self.evidence_uris {
            require_public_text("evidence_uris", uri)?;
        }
        Ok(())
    }

    /// Compute the domain-separated canonical entry hash.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError::CanonicalEncode`] if Norito encoding fails.
    pub fn entry_hash(&self) -> Result<[u8; 32], TransparencyLedgerError> {
        hash_norito(ENTRY_HASH_DOMAIN_V1, self)
    }
}

impl ModerationLedgerBlockV1 {
    /// Build a canonical block header from a set of entries.
    ///
    /// Entries are validated, deduplicated by entry id, sorted by
    /// `(sequence, entry_id)`, and hashed into a deterministic Merkle root.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError`] when the cycle metadata or entries
    /// are malformed.
    pub fn build(
        cycle_id: [u8; 16],
        cycle_start_unix: u64,
        cycle_end_unix: u64,
        generated_at_unix: u64,
        previous_block_hash: Option<[u8; 32]>,
        entries: &[ModerationLedgerEntryV1],
    ) -> Result<Self, TransparencyLedgerError> {
        require_nonzero16("cycle_id", &cycle_id)?;
        if cycle_start_unix == 0 {
            return Err(TransparencyLedgerError::InvalidTimestamp {
                field: "cycle_start_unix",
            });
        }
        if cycle_end_unix <= cycle_start_unix {
            return Err(TransparencyLedgerError::InvalidCycleWindow);
        }
        if generated_at_unix < cycle_end_unix {
            return Err(TransparencyLedgerError::InvalidGeneratedAt);
        }
        if let Some(previous) = &previous_block_hash {
            require_nonzero32("previous_block_hash", previous)?;
        }
        let sorted = sorted_entries_for_cycle(cycle_id, entries)?;
        let entry_count =
            u32::try_from(sorted.len()).map_err(|_| TransparencyLedgerError::TooManyEntries {
                count: sorted.len(),
            })?;
        let leaf_hashes = sorted
            .iter()
            .map(ModerationLedgerEntryV1::entry_hash)
            .collect::<Result<Vec<_>, _>>()?;
        let entry_root = merkle_root_from_leaf_hashes(&leaf_hashes)?;
        Ok(Self {
            version: MODERATION_LEDGER_BLOCK_VERSION_V1,
            cycle_id,
            cycle_start_unix,
            cycle_end_unix,
            generated_at_unix,
            entry_count,
            entry_root,
            previous_block_hash,
        })
    }

    /// Validate this block header.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError`] when versioning, timestamps, roots,
    /// or optional chain linkage are malformed.
    pub fn validate(&self) -> Result<(), TransparencyLedgerError> {
        if self.version != MODERATION_LEDGER_BLOCK_VERSION_V1 {
            return Err(TransparencyLedgerError::UnsupportedBlockVersion {
                expected: MODERATION_LEDGER_BLOCK_VERSION_V1,
                found: self.version,
            });
        }
        require_nonzero16("cycle_id", &self.cycle_id)?;
        if self.cycle_start_unix == 0 {
            return Err(TransparencyLedgerError::InvalidTimestamp {
                field: "cycle_start_unix",
            });
        }
        if self.cycle_end_unix <= self.cycle_start_unix {
            return Err(TransparencyLedgerError::InvalidCycleWindow);
        }
        if self.generated_at_unix < self.cycle_end_unix {
            return Err(TransparencyLedgerError::InvalidGeneratedAt);
        }
        if self.entry_count == 0 {
            return Err(TransparencyLedgerError::MissingEntries);
        }
        require_nonzero32("entry_root", &self.entry_root)?;
        if let Some(previous) = &self.previous_block_hash {
            require_nonzero32("previous_block_hash", previous)?;
        }
        Ok(())
    }

    /// Compute the domain-separated canonical block hash.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError::CanonicalEncode`] if Norito encoding fails.
    pub fn block_hash(&self) -> Result<[u8; 32], TransparencyLedgerError> {
        hash_norito(BLOCK_HASH_DOMAIN_V1, self)
    }
}

impl ModerationLedgerProofV1 {
    /// Build an inclusion proof for `entry_id` from a cycle entry set.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError`] when entries are malformed or the
    /// requested entry id is absent.
    pub fn build(
        cycle_id: [u8; 16],
        entries: &[ModerationLedgerEntryV1],
        entry_id: [u8; 16],
    ) -> Result<Self, TransparencyLedgerError> {
        let sorted = sorted_entries_for_cycle(cycle_id, entries)?;
        let leaf_hashes = sorted
            .iter()
            .map(ModerationLedgerEntryV1::entry_hash)
            .collect::<Result<Vec<_>, _>>()?;
        let Some(leaf_index) = sorted.iter().position(|entry| entry.entry_id == entry_id) else {
            return Err(TransparencyLedgerError::EntryNotFound { entry_id });
        };
        let root = merkle_root_from_leaf_hashes(&leaf_hashes)?;
        let audit_path = merkle_audit_path(&leaf_hashes, leaf_index)?;
        let leaf_index_u32 =
            u32::try_from(leaf_index).map_err(|_| TransparencyLedgerError::TooManyEntries {
                count: sorted.len(),
            })?;
        Ok(Self {
            version: MODERATION_LEDGER_PROOF_VERSION_V1,
            cycle_id,
            leaf_index: leaf_index_u32,
            entry: sorted[leaf_index].clone(),
            entry_hash: leaf_hashes[leaf_index],
            root,
            audit_path,
        })
    }

    /// Validate this proof without a block header.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError`] when versioning, cycle binding,
    /// entry hashing, or path bounds are malformed.
    pub fn validate(&self) -> Result<(), TransparencyLedgerError> {
        if self.version != MODERATION_LEDGER_PROOF_VERSION_V1 {
            return Err(TransparencyLedgerError::UnsupportedProofVersion {
                expected: MODERATION_LEDGER_PROOF_VERSION_V1,
                found: self.version,
            });
        }
        require_nonzero16("cycle_id", &self.cycle_id)?;
        self.entry.validate()?;
        if self.entry.cycle_id != self.cycle_id {
            return Err(TransparencyLedgerError::ProofCycleMismatch);
        }
        if self.audit_path.len() > MODERATION_LEDGER_MAX_PROOF_PATH_LEN {
            return Err(TransparencyLedgerError::ProofPathTooLong {
                length: self.audit_path.len(),
            });
        }
        let expected_entry_hash = self.entry.entry_hash()?;
        if self.entry_hash != expected_entry_hash {
            return Err(TransparencyLedgerError::ProofEntryHashMismatch);
        }
        require_nonzero32("root", &self.root)?;
        for node in &self.audit_path {
            require_nonzero32("audit_path.hash", &node.hash)?;
        }
        Ok(())
    }

    /// Verify this proof against a block header.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError`] if the proof or block is malformed,
    /// the proof is not bound to the block, or the recomputed root differs.
    pub fn verify_against_block(
        &self,
        block: &ModerationLedgerBlockV1,
    ) -> Result<(), TransparencyLedgerError> {
        self.validate()?;
        block.validate()?;
        if self.cycle_id != block.cycle_id {
            return Err(TransparencyLedgerError::ProofCycleMismatch);
        }
        if self.leaf_index >= block.entry_count {
            return Err(TransparencyLedgerError::ProofIndexOutOfBounds {
                leaf_index: self.leaf_index,
                entry_count: block.entry_count,
            });
        }
        verify_audit_path_shape(self.leaf_index, block.entry_count, &self.audit_path)?;
        if self.root != block.entry_root {
            return Err(TransparencyLedgerError::ProofRootMismatch);
        }
        let root = recompute_root_from_path(self.entry_hash, &self.audit_path)?;
        if root != block.entry_root {
            return Err(TransparencyLedgerError::ProofRootMismatch);
        }
        Ok(())
    }
}

impl ModerationLedgerCyclePublicationV1 {
    /// Build a canonical cycle publication from an entry set.
    ///
    /// Entries are validated, sorted deterministically by `(sequence, entry_id)`,
    /// included in the cycle block, and assigned one Merkle inclusion proof per
    /// leaf.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError`] when cycle metadata, entries, or
    /// generated proofs are malformed.
    pub fn from_entries(
        cycle_id: [u8; 16],
        cycle_start_unix: u64,
        cycle_end_unix: u64,
        generated_at_unix: u64,
        previous_block_hash: Option<[u8; 32]>,
        entries: &[ModerationLedgerEntryV1],
    ) -> Result<Self, TransparencyLedgerError> {
        let block = ModerationLedgerBlockV1::build(
            cycle_id,
            cycle_start_unix,
            cycle_end_unix,
            generated_at_unix,
            previous_block_hash,
            entries,
        )?;
        let sorted = sorted_entries_for_cycle(cycle_id, entries)?;
        let leaf_hashes = sorted
            .iter()
            .map(ModerationLedgerEntryV1::entry_hash)
            .collect::<Result<Vec<_>, _>>()?;
        let proofs = sorted
            .into_iter()
            .enumerate()
            .map(|(leaf_index, entry)| {
                let leaf_index_u32 = u32::try_from(leaf_index).map_err(|_| {
                    TransparencyLedgerError::TooManyEntries {
                        count: leaf_hashes.len(),
                    }
                })?;
                Ok(ModerationLedgerProofV1 {
                    version: MODERATION_LEDGER_PROOF_VERSION_V1,
                    cycle_id,
                    leaf_index: leaf_index_u32,
                    entry,
                    entry_hash: leaf_hashes[leaf_index],
                    root: block.entry_root,
                    audit_path: merkle_audit_path(&leaf_hashes, leaf_index)?,
                })
            })
            .collect::<Result<Vec<_>, TransparencyLedgerError>>()?;
        let publication = Self {
            version: MODERATION_LEDGER_PUBLICATION_VERSION_V1,
            block,
            proofs,
        };
        publication.validate()?;
        Ok(publication)
    }

    /// Validate this publication bundle.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError`] when the version, block, proof count,
    /// proof ordering, duplicate leaves, or proof verification fails.
    pub fn validate(&self) -> Result<(), TransparencyLedgerError> {
        if self.version != MODERATION_LEDGER_PUBLICATION_VERSION_V1 {
            return Err(TransparencyLedgerError::UnsupportedPublicationVersion {
                expected: MODERATION_LEDGER_PUBLICATION_VERSION_V1,
                found: self.version,
            });
        }
        self.block.validate()?;
        let expected = usize::try_from(self.block.entry_count).unwrap_or(usize::MAX);
        if self.proofs.len() != expected {
            return Err(TransparencyLedgerError::PublicationProofCountMismatch {
                expected,
                found: self.proofs.len(),
            });
        }
        let mut previous_leaf_index = None;
        let mut seen_leaf_indices = BTreeSet::new();
        for proof in &self.proofs {
            if let Some(previous) = previous_leaf_index
                && previous > proof.leaf_index
            {
                return Err(TransparencyLedgerError::PublicationProofsUnsorted);
            }
            if !seen_leaf_indices.insert(proof.leaf_index) {
                return Err(
                    TransparencyLedgerError::DuplicatePublicationProofLeafIndex {
                        leaf_index: proof.leaf_index,
                    },
                );
            }
            proof.verify_against_block(&self.block)?;
            previous_leaf_index = Some(proof.leaf_index);
        }
        Ok(())
    }

    /// Compute the domain-separated canonical publication hash.
    ///
    /// # Errors
    ///
    /// Returns [`TransparencyLedgerError::CanonicalEncode`] if Norito encoding fails.
    pub fn publication_hash(&self) -> Result<[u8; 32], TransparencyLedgerError> {
        hash_norito(PUBLICATION_HASH_DOMAIN_V1, self)
    }
}

/// Errors surfaced while validating transparency ledger payloads.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TransparencyLedgerError {
    /// Entry version mismatch.
    #[error("unsupported moderation ledger entry version `{found}` (expected {expected})")]
    UnsupportedEntryVersion {
        /// Expected schema version.
        expected: u16,
        /// Found schema version.
        found: u16,
    },
    /// Block version mismatch.
    #[error("unsupported moderation ledger block version `{found}` (expected {expected})")]
    UnsupportedBlockVersion {
        /// Expected schema version.
        expected: u16,
        /// Found schema version.
        found: u16,
    },
    /// Proof version mismatch.
    #[error("unsupported moderation ledger proof version `{found}` (expected {expected})")]
    UnsupportedProofVersion {
        /// Expected schema version.
        expected: u16,
        /// Found schema version.
        found: u16,
    },
    /// Publication version mismatch.
    #[error("unsupported moderation ledger publication version `{found}` (expected {expected})")]
    UnsupportedPublicationVersion {
        /// Expected schema version.
        expected: u16,
        /// Found schema version.
        found: u16,
    },
    /// Privacy aggregate version mismatch.
    #[error("unsupported moderation privacy aggregate version `{found}` (expected {expected})")]
    UnsupportedPrivacyAggregateVersion {
        /// Expected schema version.
        expected: u16,
        /// Found schema version.
        found: u16,
    },
    /// Privacy parameters version mismatch.
    #[error("unsupported moderation privacy parameters version `{found}` (expected {expected})")]
    UnsupportedPrivacyParametersVersion {
        /// Expected schema version.
        expected: u16,
        /// Found schema version.
        found: u16,
    },
    /// Proof-token issuance version mismatch.
    #[error("unsupported proof-token issuance version `{found}` (expected {expected})")]
    UnsupportedProofTokenIssuanceVersion {
        /// Expected schema version.
        expected: u16,
        /// Found schema version.
        found: u16,
    },
    /// Required identifier or digest is all zero.
    #[error("moderation ledger field `{field}` must be non-zero")]
    MissingDigest {
        /// Field name.
        field: &'static str,
    },
    /// Required text field is blank or contains NUL.
    #[error("moderation ledger field `{field}` must be non-empty public text")]
    MissingText {
        /// Field name.
        field: &'static str,
    },
    /// Timestamp field is zero.
    #[error("moderation ledger timestamp `{field}` must be non-zero")]
    InvalidTimestamp {
        /// Field name.
        field: &'static str,
    },
    /// Cycle end is not greater than cycle start.
    #[error("moderation ledger cycle end must be greater than cycle start")]
    InvalidCycleWindow,
    /// Block generation timestamp predates the cycle end.
    #[error("moderation ledger generated_at timestamp must be >= cycle end")]
    InvalidGeneratedAt,
    /// Privacy aggregate end is not greater than window start.
    #[error("moderation privacy aggregate window end must be greater than window start")]
    InvalidPrivacyAggregateWindow,
    /// Privacy aggregate generation timestamp predates the window end.
    #[error("moderation privacy aggregate generated_at timestamp must be >= window end")]
    InvalidPrivacyAggregateGeneratedAt,
    /// Entry list is empty.
    #[error("moderation ledger block requires at least one entry")]
    MissingEntries,
    /// Entry belongs to a different cycle.
    #[error("moderation ledger entry cycle id does not match the block cycle id")]
    EntryCycleMismatch,
    /// Entry id appears more than once in a cycle.
    #[error("duplicate moderation ledger entry id")]
    DuplicateEntryId {
        /// Duplicate entry id.
        entry_id: [u8; 16],
    },
    /// Too many entries for the V1 u32 count field.
    #[error("moderation ledger entry count `{count}` exceeds the V1 limit")]
    TooManyEntries {
        /// Entry count that could not be represented.
        count: usize,
    },
    /// Requested proof target was absent.
    #[error("moderation ledger entry id not found")]
    EntryNotFound {
        /// Missing entry id.
        entry_id: [u8; 16],
    },
    /// Metadata keys are not sorted.
    #[error("moderation ledger metadata keys must be sorted")]
    MetadataKeysUnsorted,
    /// Metadata key appears more than once.
    #[error("duplicate moderation ledger metadata key `{key}`")]
    DuplicateMetadataKey {
        /// Duplicate metadata key.
        key: String,
    },
    /// Required privacy parameter is missing.
    #[error("moderation privacy parameter `{field}` is required")]
    MissingPrivacyParameter {
        /// Field name.
        field: &'static str,
    },
    /// Privacy parameter is present but out of range.
    #[error("moderation privacy parameter `{field}` is invalid")]
    InvalidPrivacyParameter {
        /// Field name.
        field: &'static str,
    },
    /// Privacy parameter is not valid for the selected mode.
    #[error("moderation privacy parameter `{field}` is not valid for the selected mode")]
    UnexpectedPrivacyParameter {
        /// Field name.
        field: &'static str,
    },
    /// Privacy aggregate does not contain any metrics.
    #[error("moderation privacy aggregate requires at least one metric")]
    PrivacyAggregateMetricsMissing,
    /// Privacy aggregate metric keys are not sorted.
    #[error("moderation privacy aggregate metric keys must be sorted")]
    PrivacyAggregateMetricKeysUnsorted,
    /// Privacy aggregate metric key appears more than once.
    #[error("duplicate moderation privacy aggregate metric key `{key}`")]
    DuplicatePrivacyAggregateMetricKey {
        /// Duplicate metric key.
        key: String,
    },
    /// Proof-token issuance record does not contain entry identifiers.
    #[error("proof-token issuance requires at least one entry id")]
    ProofTokenEntryIdsMissing,
    /// Proof-token issuance entry id appears more than once.
    #[error("duplicate proof-token issuance entry id `{entry_id}`")]
    DuplicateProofTokenEntryId {
        /// Duplicate entry id.
        entry_id: String,
    },
    /// Proof-token expiry does not follow the issued timestamp.
    #[error("proof-token expiry must be greater than issued_at")]
    InvalidProofTokenExpiry,
    /// Publication proof count does not match the block entry count.
    #[error("moderation ledger publication has `{found}` proofs but block expects `{expected}`")]
    PublicationProofCountMismatch {
        /// Expected proof count.
        expected: usize,
        /// Observed proof count.
        found: usize,
    },
    /// Publication proofs are not sorted by leaf index.
    #[error("moderation ledger publication proofs must be sorted by leaf index")]
    PublicationProofsUnsorted,
    /// Publication contains two proofs for the same leaf index.
    #[error("duplicate moderation ledger publication proof for leaf index `{leaf_index}`")]
    DuplicatePublicationProofLeafIndex {
        /// Duplicate proof leaf index.
        leaf_index: u32,
    },
    /// Norito canonical encoding failed while hashing.
    #[error("failed to encode moderation ledger payload for canonical hashing")]
    CanonicalEncode,
    /// Proof path exceeds the configured verifier bound.
    #[error("moderation ledger proof path length `{length}` exceeds the verifier bound")]
    ProofPathTooLong {
        /// Observed audit-path length.
        length: usize,
    },
    /// Proof path length does not match the block entry count.
    #[error(
        "moderation ledger proof path length `{found}` does not match expected length `{expected}`"
    )]
    ProofPathLengthMismatch {
        /// Expected path length for the claimed leaf and entry count.
        expected: usize,
        /// Observed audit-path length.
        found: usize,
    },
    /// Proof path side does not match the claimed leaf index.
    #[error("moderation ledger proof path side mismatch at position `{path_position}`")]
    ProofPathIndexMismatch {
        /// Claimed leaf index.
        leaf_index: u32,
        /// Path position with the first side mismatch.
        path_position: usize,
    },
    /// Proof entry does not belong to the proof cycle.
    #[error("moderation ledger proof cycle mismatch")]
    ProofCycleMismatch,
    /// Proof entry hash does not match the embedded entry.
    #[error("moderation ledger proof entry hash mismatch")]
    ProofEntryHashMismatch,
    /// Proof leaf index is outside the block entry count.
    #[error("moderation ledger proof index `{leaf_index}` is outside entry count `{entry_count}`")]
    ProofIndexOutOfBounds {
        /// Claimed leaf index.
        leaf_index: u32,
        /// Block entry count.
        entry_count: u32,
    },
    /// Proof root does not match the block root.
    #[error("moderation ledger proof root mismatch")]
    ProofRootMismatch,
}

fn sorted_entries_for_cycle(
    cycle_id: [u8; 16],
    entries: &[ModerationLedgerEntryV1],
) -> Result<Vec<ModerationLedgerEntryV1>, TransparencyLedgerError> {
    if entries.is_empty() {
        return Err(TransparencyLedgerError::MissingEntries);
    }
    let mut seen = BTreeSet::new();
    let mut sorted = Vec::with_capacity(entries.len());
    for entry in entries {
        entry.validate()?;
        if entry.cycle_id != cycle_id {
            return Err(TransparencyLedgerError::EntryCycleMismatch);
        }
        if !seen.insert(entry.entry_id) {
            return Err(TransparencyLedgerError::DuplicateEntryId {
                entry_id: entry.entry_id,
            });
        }
        sorted.push(entry.clone());
    }
    sorted.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.entry_id.cmp(&right.entry_id))
    });
    Ok(sorted)
}

fn merkle_root_from_leaf_hashes(
    leaf_hashes: &[[u8; 32]],
) -> Result<[u8; 32], TransparencyLedgerError> {
    if leaf_hashes.is_empty() {
        return Err(TransparencyLedgerError::MissingEntries);
    }
    let mut layer = leaf_hashes.to_vec();
    while layer.len() > 1 {
        layer = next_merkle_layer(&layer);
    }
    Ok(layer[0])
}

fn merkle_audit_path(
    leaf_hashes: &[[u8; 32]],
    mut index: usize,
) -> Result<Vec<ModerationLedgerProofNodeV1>, TransparencyLedgerError> {
    if leaf_hashes.is_empty() {
        return Err(TransparencyLedgerError::MissingEntries);
    }
    let mut layer = leaf_hashes.to_vec();
    let mut path = Vec::new();
    while layer.len() > 1 {
        let sibling_index = if index.is_multiple_of(2) {
            (index + 1).min(layer.len() - 1)
        } else {
            index - 1
        };
        path.push(ModerationLedgerProofNodeV1 {
            side: if index.is_multiple_of(2) {
                ModerationLedgerProofSideV1::Right
            } else {
                ModerationLedgerProofSideV1::Left
            },
            hash: layer[sibling_index],
        });
        layer = next_merkle_layer(&layer);
        index /= 2;
    }
    Ok(path)
}

fn next_merkle_layer(layer: &[[u8; 32]]) -> Vec<[u8; 32]> {
    layer
        .chunks(2)
        .map(|chunk| {
            let left = chunk[0];
            let right = *chunk.get(1).unwrap_or(&left);
            hash_node(left, right)
        })
        .collect()
}

fn recompute_root_from_path(
    mut current: [u8; 32],
    path: &[ModerationLedgerProofNodeV1],
) -> Result<[u8; 32], TransparencyLedgerError> {
    if path.len() > MODERATION_LEDGER_MAX_PROOF_PATH_LEN {
        return Err(TransparencyLedgerError::ProofPathTooLong { length: path.len() });
    }
    for node in path {
        current = match node.side {
            ModerationLedgerProofSideV1::Left => hash_node(node.hash, current),
            ModerationLedgerProofSideV1::Right => hash_node(current, node.hash),
        };
    }
    Ok(current)
}

fn verify_audit_path_shape(
    leaf_index: u32,
    entry_count: u32,
    path: &[ModerationLedgerProofNodeV1],
) -> Result<(), TransparencyLedgerError> {
    let mut index = leaf_index as usize;
    let mut width = entry_count as usize;
    let mut expected_sides = Vec::new();
    while width > 1 {
        expected_sides.push(if index.is_multiple_of(2) {
            ModerationLedgerProofSideV1::Right
        } else {
            ModerationLedgerProofSideV1::Left
        });
        index /= 2;
        width = width.div_ceil(2);
    }
    if path.len() != expected_sides.len() {
        return Err(TransparencyLedgerError::ProofPathLengthMismatch {
            expected: expected_sides.len(),
            found: path.len(),
        });
    }
    for (path_position, (node, expected_side)) in
        path.iter().zip(expected_sides.into_iter()).enumerate()
    {
        if node.side != expected_side {
            return Err(TransparencyLedgerError::ProofPathIndexMismatch {
                leaf_index,
                path_position,
            });
        }
    }
    Ok(())
}

fn hash_node(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(MERKLE_NODE_DOMAIN_V1);
    hasher.update(&left);
    hasher.update(&right);
    *hasher.finalize().as_bytes()
}

fn hash_norito<T: Encode>(domain: &[u8], value: &T) -> Result<[u8; 32], TransparencyLedgerError> {
    let bytes = norito::to_bytes(value).map_err(|_| TransparencyLedgerError::CanonicalEncode)?;
    let mut hasher = Hasher::new();
    hasher.update(domain);
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}

fn hash_text(domain: &[u8], value: &str) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(domain);
    hasher.update(value.as_bytes());
    *hasher.finalize().as_bytes()
}

fn require_nonzero16(field: &'static str, value: &[u8; 16]) -> Result<(), TransparencyLedgerError> {
    if value.iter().all(|byte| *byte == 0) {
        return Err(TransparencyLedgerError::MissingDigest { field });
    }
    Ok(())
}

fn require_nonzero32(field: &'static str, value: &[u8; 32]) -> Result<(), TransparencyLedgerError> {
    if value.iter().all(|byte| *byte == 0) {
        return Err(TransparencyLedgerError::MissingDigest { field });
    }
    Ok(())
}

fn require_public_text(field: &'static str, value: &str) -> Result<(), TransparencyLedgerError> {
    if value.trim().is_empty() || value.contains('\0') {
        return Err(TransparencyLedgerError::MissingText { field });
    }
    Ok(())
}

fn require_positive_parameter(
    field: &'static str,
    value: Option<u64>,
) -> Result<u64, TransparencyLedgerError> {
    match value {
        Some(value) if value > 0 => Ok(value),
        Some(_) => Err(TransparencyLedgerError::InvalidPrivacyParameter { field }),
        None => Err(TransparencyLedgerError::MissingPrivacyParameter { field }),
    }
}

fn require_positive_optional_parameter(
    field: &'static str,
    value: Option<u64>,
) -> Result<(), TransparencyLedgerError> {
    if matches!(value, Some(0)) {
        return Err(TransparencyLedgerError::InvalidPrivacyParameter { field });
    }
    Ok(())
}

fn require_delta_parameter(value: Option<u64>) -> Result<u64, TransparencyLedgerError> {
    match value {
        Some(value) if value <= MODERATION_PRIVACY_DELTA_PPB_MAX => Ok(value),
        Some(_) => Err(TransparencyLedgerError::InvalidPrivacyParameter { field: "delta_ppb" }),
        None => Err(TransparencyLedgerError::MissingPrivacyParameter { field: "delta_ppb" }),
    }
}

fn require_absent_parameter(
    field: &'static str,
    value: Option<u64>,
) -> Result<(), TransparencyLedgerError> {
    if value.is_some() {
        return Err(TransparencyLedgerError::UnexpectedPrivacyParameter { field });
    }
    Ok(())
}

fn validate_metadata(
    metadata: &[ModerationLedgerMetadataV1],
) -> Result<(), TransparencyLedgerError> {
    let mut last_key: Option<&str> = None;
    let mut seen_keys = BTreeSet::new();
    for item in metadata {
        require_public_text("metadata.key", &item.key)?;
        require_public_text("metadata.value", &item.value)?;
        if let Some(last) = last_key
            && last > item.key.as_str()
        {
            return Err(TransparencyLedgerError::MetadataKeysUnsorted);
        }
        if !seen_keys.insert(item.key.as_str()) {
            return Err(TransparencyLedgerError::DuplicateMetadataKey {
                key: item.key.clone(),
            });
        }
        last_key = Some(item.key.as_str());
    }
    Ok(())
}

fn validate_privacy_metrics(
    metrics: &[ModerationPrivacyAggregateMetricV1],
) -> Result<(), TransparencyLedgerError> {
    if metrics.is_empty() {
        return Err(TransparencyLedgerError::PrivacyAggregateMetricsMissing);
    }
    let mut last_key: Option<&str> = None;
    let mut seen_keys = BTreeSet::new();
    for item in metrics {
        require_public_text("metrics.key", &item.key)?;
        require_public_text("metrics.unit", &item.unit)?;
        if let Some(last) = last_key
            && last > item.key.as_str()
        {
            return Err(TransparencyLedgerError::PrivacyAggregateMetricKeysUnsorted);
        }
        if !seen_keys.insert(item.key.as_str()) {
            return Err(
                TransparencyLedgerError::DuplicatePrivacyAggregateMetricKey {
                    key: item.key.clone(),
                },
            );
        }
        last_key = Some(item.key.as_str());
    }
    Ok(())
}

fn validate_entry_ids(entry_ids: &[String]) -> Result<(), TransparencyLedgerError> {
    if entry_ids.is_empty() {
        return Err(TransparencyLedgerError::ProofTokenEntryIdsMissing);
    }
    let mut seen = BTreeSet::new();
    for entry_id in entry_ids {
        require_public_text("entry_ids", entry_id)?;
        if !seen.insert(entry_id.as_str()) {
            return Err(TransparencyLedgerError::DuplicateProofTokenEntryId {
                entry_id: entry_id.clone(),
            });
        }
    }
    Ok(())
}

fn insert_metadata(
    metadata: &mut BTreeMap<String, String>,
    key: &str,
    value: String,
) -> Result<(), TransparencyLedgerError> {
    if metadata.insert(key.to_string(), value).is_some() {
        return Err(TransparencyLedgerError::DuplicateMetadataKey {
            key: key.to_string(),
        });
    }
    Ok(())
}

fn privacy_mode_label(mode: ModerationPrivacyModeV1) -> &'static str {
    match mode {
        ModerationPrivacyModeV1::DifferentialPrivacy => "differential_privacy",
        ModerationPrivacyModeV1::Suppression => "suppression",
        ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression => {
            "differential_privacy_with_suppression"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn cycle_id() -> [u8; 16] {
        *b"cycle-2026-wk-01"
    }

    fn entry(entry_seed: u8, sequence: u64) -> ModerationLedgerEntryV1 {
        ModerationLedgerEntryV1 {
            version: MODERATION_LEDGER_ENTRY_VERSION_V1,
            cycle_id: cycle_id(),
            entry_id: [entry_seed; 16],
            sequence,
            occurred_at_unix: 1_767_225_600 + sequence,
            kind: ModerationLedgerEntryKindV1::GarEnforcementReceipt,
            subject: format!("gar-receipt-{entry_seed}"),
            subject_digest: digest(entry_seed),
            payload_digest: digest(entry_seed + 1),
            summary_digest: digest(entry_seed + 2),
            policy_digest: Some(digest(entry_seed + 3)),
            evidence_uris: vec![format!("sora://transparency/{entry_seed}")],
            metadata: vec![
                ModerationLedgerMetadataV1 {
                    key: "action".to_string(),
                    value: "geo_fence".to_string(),
                },
                ModerationLedgerMetadataV1 {
                    key: "source".to_string(),
                    value: "gar".to_string(),
                },
            ],
        }
    }

    fn privacy_aggregate() -> ModerationPrivacyAggregateV1 {
        ModerationPrivacyAggregateV1 {
            version: MODERATION_PRIVACY_AGGREGATE_VERSION_V1,
            aggregate_id: "sfm4c-weekly-jurisdiction-a".to_string(),
            window_start_unix: 1_767_225_600,
            window_end_unix: 1_767_830_400,
            generated_at_unix: 1_767_830_401,
            population_label: "jurisdiction-a".to_string(),
            population_digest: digest(0xA0),
            privacy: ModerationPrivacyParametersV1 {
                version: MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
                mode: ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression,
                epsilon_micros: Some(750_000),
                delta_ppb: Some(10),
                noise_scale_micros: Some(1_250_000),
                suppression_threshold: Some(25),
                suppressed_count: 3,
            },
            source_event_count: 128,
            source_payload_digest: digest(0xA1),
            metrics: vec![
                ModerationPrivacyAggregateMetricV1 {
                    key: "appeals_upheld".to_string(),
                    value: 4,
                    unit: "count".to_string(),
                },
                ModerationPrivacyAggregateMetricV1 {
                    key: "moderation_actions".to_string(),
                    value: 29,
                    unit: "count".to_string(),
                },
            ],
            policy_digest: Some(digest(0xA2)),
            metadata: vec![ModerationLedgerMetadataV1 {
                key: "publisher".to_string(),
                value: "sfm4c".to_string(),
            }],
        }
    }

    fn proof_token_issuance() -> ProofTokenIssuanceV1 {
        ProofTokenIssuanceV1 {
            version: PROOF_TOKEN_ISSUANCE_VERSION_V1,
            token_id: [0x51; 16],
            issued_at_unix: 1_767_830_401,
            expires_at_unix: Some(1_767_831_001),
            moderation_action_code: 1,
            signer_key: digest(0x52),
            token_blake3: digest(0x53),
            blinded_digest: digest(0x54),
            entry_ids: vec!["denylist/global".to_string(), "gar/rule/42".to_string()],
            evidence_digest: Some(digest(0x55)),
            policy_digest: Some(digest(0x56)),
            metadata: vec![ModerationLedgerMetadataV1 {
                key: "issuer".to_string(),
                value: "gateway-a".to_string(),
            }],
        }
    }

    #[test]
    fn transparency_entry_block_and_proof_round_trip_via_norito() {
        let entries = vec![entry(0x22, 2), entry(0x11, 1), entry(0x33, 3)];
        let block = ModerationLedgerBlockV1::build(
            cycle_id(),
            1_767_225_600,
            1_767_830_400,
            1_767_830_401,
            None,
            &entries,
        )
        .expect("block builds");
        let proof =
            ModerationLedgerProofV1::build(cycle_id(), &entries, [0x22; 16]).expect("proof builds");
        proof
            .verify_against_block(&block)
            .expect("proof verifies against block");

        let block_bytes = norito::to_bytes(&block).expect("block encodes");
        let decoded_block: ModerationLedgerBlockV1 =
            norito::decode_from_bytes(&block_bytes).expect("block decodes");
        assert_eq!(decoded_block, block);

        let proof_bytes = norito::to_bytes(&proof).expect("proof encodes");
        let decoded_proof: ModerationLedgerProofV1 =
            norito::decode_from_bytes(&proof_bytes).expect("proof decodes");
        assert_eq!(decoded_proof, proof);
    }

    #[test]
    fn transparency_publication_round_trip_via_norito() {
        let entries = vec![entry(0x22, 2), entry(0x11, 1), entry(0x33, 3)];
        let publication = ModerationLedgerCyclePublicationV1::from_entries(
            cycle_id(),
            1_767_225_600,
            1_767_830_400,
            1_767_830_401,
            Some(digest(0xAA)),
            &entries,
        )
        .expect("publication builds");
        assert_eq!(publication.block.entry_count, 3);
        assert_eq!(
            publication
                .proofs
                .iter()
                .map(|proof| proof.leaf_index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        for proof in &publication.proofs {
            proof
                .verify_against_block(&publication.block)
                .expect("publication proof verifies");
        }

        let publication_bytes = norito::to_bytes(&publication).expect("publication encodes");
        let decoded_publication: ModerationLedgerCyclePublicationV1 =
            norito::decode_from_bytes(&publication_bytes).expect("publication decodes");
        assert_eq!(decoded_publication, publication);
        assert_eq!(
            decoded_publication
                .publication_hash()
                .expect("decoded hashes"),
            publication.publication_hash().expect("publication hashes")
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn transparency_proof_round_trip_via_json() {
        let entries = vec![entry(0x22, 2), entry(0x11, 1)];
        let proof =
            ModerationLedgerProofV1::build(cycle_id(), &entries, [0x11; 16]).expect("proof builds");
        let json = norito::json::to_vec(&proof).expect("proof json encodes");
        let decoded: ModerationLedgerProofV1 =
            norito::json::from_slice(&json).expect("proof json decodes");
        assert_eq!(decoded, proof);
    }

    #[test]
    fn transparency_entries_are_sorted_before_rooting() {
        let sorted = vec![entry(0x11, 1), entry(0x22, 2), entry(0x33, 3)];
        let unsorted = vec![entry(0x33, 3), entry(0x11, 1), entry(0x22, 2)];
        let sorted_block = ModerationLedgerBlockV1::build(
            cycle_id(),
            1_767_225_600,
            1_767_830_400,
            1_767_830_401,
            None,
            &sorted,
        )
        .expect("sorted block");
        let unsorted_block = ModerationLedgerBlockV1::build(
            cycle_id(),
            1_767_225_600,
            1_767_830_400,
            1_767_830_401,
            None,
            &unsorted,
        )
        .expect("unsorted block");
        assert_eq!(sorted_block.entry_root, unsorted_block.entry_root);
    }

    #[test]
    fn transparency_proof_rejects_tampered_entry() {
        let entries = vec![entry(0x22, 2), entry(0x11, 1)];
        let block = ModerationLedgerBlockV1::build(
            cycle_id(),
            1_767_225_600,
            1_767_830_400,
            1_767_830_401,
            None,
            &entries,
        )
        .expect("block builds");
        let mut proof =
            ModerationLedgerProofV1::build(cycle_id(), &entries, [0x11; 16]).expect("proof builds");
        proof.entry.summary_digest = digest(0xF0);
        assert_eq!(
            proof.verify_against_block(&block),
            Err(TransparencyLedgerError::ProofEntryHashMismatch)
        );
    }

    #[test]
    fn transparency_proof_rejects_wrong_leaf_index() {
        let entries = vec![entry(0x22, 2), entry(0x11, 1)];
        let block = ModerationLedgerBlockV1::build(
            cycle_id(),
            1_767_225_600,
            1_767_830_400,
            1_767_830_401,
            None,
            &entries,
        )
        .expect("block builds");
        let mut proof =
            ModerationLedgerProofV1::build(cycle_id(), &entries, [0x11; 16]).expect("proof builds");
        proof.leaf_index = 1;
        assert_eq!(
            proof.verify_against_block(&block),
            Err(TransparencyLedgerError::ProofPathIndexMismatch {
                leaf_index: 1,
                path_position: 0,
            })
        );
    }

    #[test]
    fn transparency_publication_rejects_missing_proof() {
        let entries = vec![entry(0x22, 2), entry(0x11, 1)];
        let mut publication = ModerationLedgerCyclePublicationV1::from_entries(
            cycle_id(),
            1_767_225_600,
            1_767_830_400,
            1_767_830_401,
            None,
            &entries,
        )
        .expect("publication builds");
        publication.proofs.pop();
        assert_eq!(
            publication.validate(),
            Err(TransparencyLedgerError::PublicationProofCountMismatch {
                expected: 2,
                found: 1,
            })
        );
    }

    #[test]
    fn transparency_publication_rejects_unsorted_proofs() {
        let entries = vec![entry(0x22, 2), entry(0x11, 1)];
        let mut publication = ModerationLedgerCyclePublicationV1::from_entries(
            cycle_id(),
            1_767_225_600,
            1_767_830_400,
            1_767_830_401,
            None,
            &entries,
        )
        .expect("publication builds");
        publication.proofs.swap(0, 1);
        assert_eq!(
            publication.validate(),
            Err(TransparencyLedgerError::PublicationProofsUnsorted)
        );
    }

    #[test]
    fn transparency_publication_rejects_duplicate_leaf_index() {
        let entries = vec![entry(0x22, 2), entry(0x11, 1)];
        let mut publication = ModerationLedgerCyclePublicationV1::from_entries(
            cycle_id(),
            1_767_225_600,
            1_767_830_400,
            1_767_830_401,
            None,
            &entries,
        )
        .expect("publication builds");
        publication.proofs[1].leaf_index = 0;
        assert_eq!(
            publication.validate(),
            Err(TransparencyLedgerError::DuplicatePublicationProofLeafIndex { leaf_index: 0 })
        );
    }

    #[test]
    fn transparency_block_rejects_duplicate_entries() {
        let entries = vec![entry(0x11, 1), entry(0x11, 2)];
        assert!(matches!(
            ModerationLedgerBlockV1::build(
                cycle_id(),
                1_767_225_600,
                1_767_830_400,
                1_767_830_401,
                None,
                &entries,
            ),
            Err(TransparencyLedgerError::DuplicateEntryId { .. })
        ));
    }

    #[test]
    fn transparency_entry_rejects_unsorted_metadata() {
        let mut candidate = entry(0x11, 1);
        candidate.metadata = vec![
            ModerationLedgerMetadataV1 {
                key: "source".to_string(),
                value: "gar".to_string(),
            },
            ModerationLedgerMetadataV1 {
                key: "action".to_string(),
                value: "geo_fence".to_string(),
            },
        ];
        assert_eq!(
            candidate.validate(),
            Err(TransparencyLedgerError::MetadataKeysUnsorted)
        );
    }

    #[test]
    fn privacy_aggregate_round_trips_and_converts_to_ledger_entry() {
        let aggregate = privacy_aggregate();
        aggregate.validate().expect("aggregate validates");
        let aggregate_bytes = norito::to_bytes(&aggregate).expect("aggregate encodes");
        let decoded: ModerationPrivacyAggregateV1 =
            norito::decode_from_bytes(&aggregate_bytes).expect("aggregate decodes");
        assert_eq!(decoded, aggregate);
        assert_eq!(
            decoded.aggregate_hash().expect("decoded hashes"),
            aggregate.aggregate_hash().expect("aggregate hashes")
        );

        let aggregate_entry = aggregate
            .to_ledger_entry(cycle_id(), [0x44; 16], 4)
            .expect("aggregate converts");
        assert_eq!(
            aggregate_entry.kind,
            ModerationLedgerEntryKindV1::PrivacyAggregate
        );
        assert_eq!(aggregate_entry.subject, aggregate.aggregate_id);
        assert_eq!(
            aggregate_entry.payload_digest,
            aggregate.aggregate_hash().expect("aggregate hashes")
        );
        assert_eq!(
            aggregate_entry.summary_digest,
            aggregate_entry.payload_digest
        );
        assert_eq!(aggregate_entry.policy_digest, aggregate.policy_digest);
        assert_eq!(
            aggregate_entry
                .metadata
                .iter()
                .map(|item| item.key.as_str())
                .collect::<Vec<_>>(),
            vec![
                "aggregate_id",
                "delta_ppb",
                "epsilon_micros",
                "noise_scale_micros",
                "population_label",
                "privacy_mode",
                "publisher",
                "source_event_count",
                "suppressed_count",
                "suppression_threshold",
                "window_end_unix",
                "window_start_unix",
            ]
        );

        let entries = vec![entry(0x22, 2), aggregate_entry];
        let publication = ModerationLedgerCyclePublicationV1::from_entries(
            cycle_id(),
            1_767_225_600,
            1_767_830_400,
            1_767_830_401,
            None,
            &entries,
        )
        .expect("publication builds with aggregate entry");
        assert_eq!(publication.block.entry_count, 2);
    }

    #[test]
    fn privacy_aggregate_rejects_missing_dp_parameters() {
        let mut aggregate = privacy_aggregate();
        aggregate.privacy.mode = ModerationPrivacyModeV1::DifferentialPrivacy;
        aggregate.privacy.epsilon_micros = None;
        aggregate.privacy.suppression_threshold = None;
        aggregate.privacy.suppressed_count = 0;
        assert_eq!(
            aggregate.validate(),
            Err(TransparencyLedgerError::MissingPrivacyParameter {
                field: "epsilon_micros",
            })
        );
    }

    #[test]
    fn privacy_aggregate_rejects_unsorted_metrics() {
        let mut aggregate = privacy_aggregate();
        aggregate.metrics = vec![
            ModerationPrivacyAggregateMetricV1 {
                key: "z_metric".to_string(),
                value: 1,
                unit: "count".to_string(),
            },
            ModerationPrivacyAggregateMetricV1 {
                key: "a_metric".to_string(),
                value: 1,
                unit: "count".to_string(),
            },
        ];
        assert_eq!(
            aggregate.validate(),
            Err(TransparencyLedgerError::PrivacyAggregateMetricKeysUnsorted)
        );
    }

    #[test]
    fn privacy_aggregate_rejects_suppression_without_threshold() {
        let mut aggregate = privacy_aggregate();
        aggregate.privacy.mode = ModerationPrivacyModeV1::Suppression;
        aggregate.privacy.epsilon_micros = None;
        aggregate.privacy.delta_ppb = None;
        aggregate.privacy.noise_scale_micros = None;
        aggregate.privacy.suppression_threshold = None;
        assert_eq!(
            aggregate.validate(),
            Err(TransparencyLedgerError::MissingPrivacyParameter {
                field: "suppression_threshold",
            })
        );
    }

    #[test]
    fn proof_token_issuance_round_trips_and_converts_to_ledger_entry() {
        let issuance = proof_token_issuance();
        issuance.validate().expect("issuance validates");
        let encoded = norito::to_bytes(&issuance).expect("issuance encodes");
        let decoded: ProofTokenIssuanceV1 =
            norito::decode_from_bytes(&encoded).expect("issuance decodes");
        assert_eq!(decoded, issuance);
        assert_eq!(
            decoded.issuance_hash().expect("decoded hashes"),
            issuance.issuance_hash().expect("issuance hashes")
        );

        let entry = issuance
            .to_ledger_entry(cycle_id(), [0x57; 16], 5)
            .expect("issuance converts");
        assert_eq!(entry.kind, ModerationLedgerEntryKindV1::ProofTokenIssuance);
        assert_eq!(
            entry.subject,
            format!("proof-token:{}", hex::encode([0x51; 16]))
        );
        assert_eq!(
            entry.payload_digest,
            issuance.issuance_hash().expect("issuance hashes")
        );
        assert_eq!(entry.policy_digest, issuance.policy_digest);
        let token_id_hex = hex::encode([0x51; 16]);
        assert_eq!(
            entry
                .metadata
                .iter()
                .find(|item| item.key == "token_id_hex")
                .map(|item| item.value.as_str()),
            Some(token_id_hex.as_str())
        );
    }

    #[test]
    fn proof_token_issuance_rejects_duplicate_entry_ids_and_bad_expiry() {
        let mut issuance = proof_token_issuance();
        issuance.entry_ids.push("denylist/global".to_string());
        assert_eq!(
            issuance.validate(),
            Err(TransparencyLedgerError::DuplicateProofTokenEntryId {
                entry_id: "denylist/global".to_string()
            })
        );

        let mut issuance = proof_token_issuance();
        issuance.expires_at_unix = Some(issuance.issued_at_unix);
        assert_eq!(
            issuance.validate(),
            Err(TransparencyLedgerError::InvalidProofTokenExpiry)
        );
    }
}
