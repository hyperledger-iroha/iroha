#![allow(unexpected_cfgs)]
//! Retention precedence helpers for SoraFS manifests.
use core::fmt;
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use thiserror::Error;
use crate::{ManifestV1, MetadataEntry};
/// Schema version for [`RetentionSourceV1`].
pub const RETENTION_SOURCE_VERSION_V1: u8 = 1;
/// Manifest metadata key for a deal-driven retention cap.
pub const RETENTION_DEAL_END_EPOCH_KEY: &str = "sorafs.retention.deal_end_epoch";
/// Manifest metadata key for a governance-imposed retention cap.
pub const RETENTION_GOVERNANCE_CAP_EPOCH_KEY: &str = "sorafs.retention.governance_cap_epoch";
/// Errors raised while parsing retention metadata.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RetentionMetadataError {
    /// Unsupported persisted retention-source schema version.
    #[error("unsupported retention source version {found}")]
    UnsupportedVersion { found: u8 },
    /// Duplicate retention metadata key.
    #[error("duplicate retention metadata key `{key}`")]
    DuplicateKey { key: String },
    /// Retention metadata value failed to parse as an unsigned integer.
    #[error("retention metadata `{key}` must be an unsigned integer (got `{value}`)")]
    InvalidValue { key: String, value: String },
    /// Optional epoch fields must use `None`, rather than `Some(0)`, for no cap.
    #[error("retention source field `{field}` must encode an absent cap as `None`, not zero")]
    NonCanonicalZeroEpoch { field: &'static str },
    /// Persisted derived fields do not match the canonical precedence result.
    #[error("retention source derived fields do not match their epoch inputs")]
    DerivedFieldsMismatch,
}
/// Retention sources applied to compute the effective expiry epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[norito(tag = "source", rename_all = "snake_case")]
pub enum RetentionSourceKindV1 {
    /// No retention constraints applied (unbounded).
    Unbounded,
    /// Pin policy `retention_epoch`.
    PinPolicy,
    /// Deal end epoch constraint.
    DealEnd,
    /// Governance retention cap constraint.
    GovernanceCap,
}
impl RetentionSourceKindV1 {
    /// Canonical string label for the source.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Unbounded => "unbounded",
            Self::PinPolicy => "pin_policy",
            Self::DealEnd => "deal_end",
            Self::GovernanceCap => "governance_cap",
        }
    }
}
impl fmt::Display for RetentionSourceKindV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
/// Retention-source record persisted alongside manifest metadata.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct RetentionSourceV1 {
    /// Schema version (`RETENTION_SOURCE_VERSION_V1`).
    pub version: u8,
    /// Pin-policy retention epoch from the manifest.
    pub pin_policy_epoch: u64,
    /// Optional deal end epoch supplied via metadata.
    #[norito(default)]
    pub deal_end_epoch: Option<u64>,
    /// Optional governance cap epoch supplied via metadata.
    #[norito(default)]
    pub governance_cap_epoch: Option<u64>,
    /// Effective retention epoch after applying precedence rules.
    pub effective_epoch: u64,
    /// Sources that matched the effective minimum epoch.
    pub sources: Vec<RetentionSourceKindV1>,
}
impl RetentionSourceV1 {
    /// Compute retention precedence from explicit inputs.
    pub fn compute(
        pin_policy_epoch: u64,
        deal_end_epoch: Option<u64>,
        governance_cap_epoch: Option<u64>,
    ) -> Result<Self, RetentionMetadataError> {
        validate_optional_epoch("deal_end_epoch", deal_end_epoch)?;
        validate_optional_epoch("governance_cap_epoch", governance_cap_epoch)?;
        Ok(Self::compute_canonical(
            pin_policy_epoch,
            deal_end_epoch,
            governance_cap_epoch,
        ))
    }
    fn compute_canonical(
        pin_policy_epoch: u64,
        deal_end_epoch: Option<u64>,
        governance_cap_epoch: Option<u64>,
    ) -> Self {
        let mut candidates = Vec::new();
        if pin_policy_epoch != 0 {
            candidates.push((RetentionSourceKindV1::PinPolicy, pin_policy_epoch));
        }
        if let Some(epoch) = deal_end_epoch {
            candidates.push((RetentionSourceKindV1::DealEnd, epoch));
        }
        if let Some(epoch) = governance_cap_epoch {
            candidates.push((RetentionSourceKindV1::GovernanceCap, epoch));
        }
        if candidates.is_empty() {
            return Self {
                version: RETENTION_SOURCE_VERSION_V1,
                pin_policy_epoch,
                deal_end_epoch,
                governance_cap_epoch,
                effective_epoch: 0,
                sources: vec![RetentionSourceKindV1::Unbounded],
            };
        }
        let min_epoch = candidates
            .iter()
            .map(|(_, epoch)| *epoch)
            .min()
            .unwrap_or(0);
        let mut sources = Vec::new();
        if deal_end_epoch.is_some_and(|epoch| epoch == min_epoch) {
            sources.push(RetentionSourceKindV1::DealEnd);
        }
        if governance_cap_epoch.is_some_and(|epoch| epoch == min_epoch) {
            sources.push(RetentionSourceKindV1::GovernanceCap);
        }
        if pin_policy_epoch == min_epoch {
            sources.push(RetentionSourceKindV1::PinPolicy);
        }
        Self {
            version: RETENTION_SOURCE_VERSION_V1,
            pin_policy_epoch,
            deal_end_epoch,
            governance_cap_epoch,
            effective_epoch: min_epoch,
            sources,
        }
    }
    /// Validate a decoded retention source, including all derived fields.
    pub fn validate(&self) -> Result<(), RetentionMetadataError> {
        if self.version != RETENTION_SOURCE_VERSION_V1 {
            return Err(RetentionMetadataError::UnsupportedVersion {
                found: self.version,
            });
        }
        validate_optional_epoch("deal_end_epoch", self.deal_end_epoch)?;
        validate_optional_epoch("governance_cap_epoch", self.governance_cap_epoch)?;
        let expected = Self::compute_canonical(
            self.pin_policy_epoch,
            self.deal_end_epoch,
            self.governance_cap_epoch,
        );
        if self.effective_epoch != expected.effective_epoch || self.sources != expected.sources {
            return Err(RetentionMetadataError::DerivedFieldsMismatch);
        }
        Ok(())
    }
    /// Compute retention precedence directly from a manifest's metadata.
    pub fn from_manifest(manifest: &ManifestV1) -> Result<Self, RetentionMetadataError> {
        let (deal_end_epoch, governance_cap_epoch) = parse_retention_metadata(&manifest.metadata)?;
        let source = Self::compute(
            manifest.pin_policy.retention_epoch,
            deal_end_epoch,
            governance_cap_epoch,
        )?;
        source.validate()?;
        Ok(source)
    }
    /// Effective retention epoch after applying precedence rules.
    #[must_use]
    pub const fn effective_epoch(&self) -> u64 {
        self.effective_epoch
    }
}
fn parse_retention_metadata(
    entries: &[MetadataEntry],
) -> Result<(Option<u64>, Option<u64>), RetentionMetadataError> {
    let mut deal_end_epoch = None;
    let mut governance_cap_epoch = None;
    for entry in entries {
        if entry.key == RETENTION_DEAL_END_EPOCH_KEY {
            if deal_end_epoch.is_some() {
                return Err(RetentionMetadataError::DuplicateKey {
                    key: entry.key.clone(),
                });
            }
            deal_end_epoch = parse_epoch_value(&entry.key, &entry.value)?;
        } else if entry.key == RETENTION_GOVERNANCE_CAP_EPOCH_KEY {
            if governance_cap_epoch.is_some() {
                return Err(RetentionMetadataError::DuplicateKey {
                    key: entry.key.clone(),
                });
            }
            governance_cap_epoch = parse_epoch_value(&entry.key, &entry.value)?;
        }
    }
    Ok((deal_end_epoch, governance_cap_epoch))
}
fn parse_epoch_value(key: &str, value: &str) -> Result<Option<u64>, RetentionMetadataError> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(RetentionMetadataError::InvalidValue {
            key: key.to_string(),
            value: value.to_string(),
        });
    }
    let parsed = value
        .parse::<u64>()
        .map_err(|_| RetentionMetadataError::InvalidValue {
            key: key.to_string(),
            value: value.to_string(),
        })?;
    if parsed == 0 {
        Ok(None)
    } else {
        Ok(Some(parsed))
    }
}
fn validate_optional_epoch(
    field: &'static str,
    epoch: Option<u64>,
) -> Result<(), RetentionMetadataError> {
    if epoch == Some(0) {
        return Err(RetentionMetadataError::NonCanonicalZeroEpoch { field });
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DagCodecId, ManifestBuilder, PinPolicy, StorageClass};
    use sorafs_chunker::ChunkProfile;
    fn manifest_with_metadata(entries: Vec<MetadataEntry>, retention_epoch: u64) -> ManifestV1 {
        ManifestBuilder::new()
            .root_cid(crate::canonical_manifest_root_cid([0xAA; 32]))
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(ChunkProfile::DEFAULT, crate::BLAKE3_256_MULTIHASH_CODE)
            .chunk_digest_sha3_256([0xAC; 32])
            .por_root([0xAD; 32])
            .content_length(128)
            .car_digest([0xAB; 32])
            .car_size(256)
            .pin_policy(PinPolicy {
                min_replicas: 3,
                storage_class: StorageClass::Hot,
                retention_epoch,
            })
            .extend_metadata(entries.into_iter().map(|entry| (entry.key, entry.value)))
            .build()
            .expect("build manifest")
    }
    #[test]
    fn retention_precedence_prefers_minimum_epoch() {
        let manifest = manifest_with_metadata(
            vec![
                MetadataEntry {
                    key: RETENTION_DEAL_END_EPOCH_KEY.into(),
                    value: "50".into(),
                },
                MetadataEntry {
                    key: RETENTION_GOVERNANCE_CAP_EPOCH_KEY.into(),
                    value: "90".into(),
                },
            ],
            120,
        );
        let source = RetentionSourceV1::from_manifest(&manifest).expect("retention source");
        assert_eq!(source.effective_epoch, 50);
        assert_eq!(source.sources, vec![RetentionSourceKindV1::DealEnd]);
    }
    #[test]
    fn retention_precedence_handles_unbounded() {
        let manifest = manifest_with_metadata(Vec::new(), 0);
        let source = RetentionSourceV1::from_manifest(&manifest).expect("retention source");
        assert_eq!(source.effective_epoch, 0);
        assert_eq!(source.sources, vec![RetentionSourceKindV1::Unbounded]);
    }
    #[test]
    fn retention_precedence_records_matching_sources() {
        let manifest = manifest_with_metadata(
            vec![MetadataEntry {
                key: RETENTION_GOVERNANCE_CAP_EPOCH_KEY.into(),
                value: "100".into(),
            }],
            100,
        );
        let source = RetentionSourceV1::from_manifest(&manifest).expect("retention source");
        assert_eq!(source.effective_epoch, 100);
        assert_eq!(
            source.sources,
            vec![
                RetentionSourceKindV1::GovernanceCap,
                RetentionSourceKindV1::PinPolicy
            ]
        );
    }
    #[test]
    fn retention_precedence_rejects_duplicate_keys() {
        let manifest = manifest_with_metadata(
            vec![
                MetadataEntry {
                    key: RETENTION_DEAL_END_EPOCH_KEY.into(),
                    value: "10".into(),
                },
                MetadataEntry {
                    key: RETENTION_DEAL_END_EPOCH_KEY.into(),
                    value: "20".into(),
                },
            ],
            30,
        );
        let err = RetentionSourceV1::from_manifest(&manifest).expect_err("duplicate key");
        assert!(matches!(err, RetentionMetadataError::DuplicateKey { .. }));
    }
    #[test]
    fn retention_precedence_rejects_invalid_values() {
        let manifest = manifest_with_metadata(
            vec![MetadataEntry {
                key: RETENTION_GOVERNANCE_CAP_EPOCH_KEY.into(),
                value: "not-a-number".into(),
            }],
            30,
        );
        let err = RetentionSourceV1::from_manifest(&manifest).expect_err("invalid value");
        assert!(matches!(err, RetentionMetadataError::InvalidValue { .. }));
    }
    #[test]
    fn retention_precedence_rejects_noncanonical_integer_spellings() {
        for value in [" 10", "10 ", "+10", "010", "1_0", "\u{ff11}0", ""] {
            let manifest = manifest_with_metadata(
                vec![MetadataEntry {
                    key: RETENTION_DEAL_END_EPOCH_KEY.into(),
                    value: value.into(),
                }],
                30,
            );
            assert!(matches!(
                RetentionSourceV1::from_manifest(&manifest),
                Err(RetentionMetadataError::InvalidValue { .. })
            ));
        }
    }
    #[test]
    fn retention_precedence_canonical_zero_means_no_optional_cap() {
        let manifest = manifest_with_metadata(
            vec![MetadataEntry {
                key: RETENTION_DEAL_END_EPOCH_KEY.into(),
                value: "0".into(),
            }],
            30,
        );
        let source = RetentionSourceV1::from_manifest(&manifest).expect("retention source");
        assert_eq!(source.deal_end_epoch, None);
        assert_eq!(source.effective_epoch, 30);
        assert_eq!(source.sources, vec![RetentionSourceKindV1::PinPolicy]);
    }
    #[test]
    fn retention_source_rejects_noncanonical_optional_zero() {
        let err = RetentionSourceV1::compute(30, Some(0), None).expect_err("optional zero");
        assert!(matches!(
            err,
            RetentionMetadataError::NonCanonicalZeroEpoch {
                field: "deal_end_epoch"
            }
        ));
    }
    #[test]
    fn retention_source_validation_recomputes_derived_fields() {
        let canonical =
            RetentionSourceV1::compute(30, Some(10), Some(20)).expect("canonical retention source");
        canonical.validate().expect("valid source");
        let mut wrong_epoch = canonical.clone();
        wrong_epoch.effective_epoch = 20;
        assert_eq!(
            wrong_epoch.validate(),
            Err(RetentionMetadataError::DerivedFieldsMismatch)
        );
        let mut wrong_sources = canonical.clone();
        wrong_sources.sources = vec![RetentionSourceKindV1::PinPolicy];
        assert_eq!(
            wrong_sources.validate(),
            Err(RetentionMetadataError::DerivedFieldsMismatch)
        );
        let mut unsupported = canonical;
        unsupported.version = RETENTION_SOURCE_VERSION_V1 + 1;
        assert!(matches!(
            unsupported.validate(),
            Err(RetentionMetadataError::UnsupportedVersion { .. })
        ));
    }
}
