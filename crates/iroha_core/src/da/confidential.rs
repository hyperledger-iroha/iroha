//! Confidential-compute lane validation helpers.
//!
//! Lanes marked as confidential compute rely on `SoraFS` tickets instead of
//! embedding payload bytes in the public DA spool. We enforce that the lane
//! declares a policy and that commitments carry non-zero digests/tickets.

use iroha_config::parameters::actual::LaneConfig as ConfigLaneConfig;
use iroha_data_model::{
    da::{
        commitment::DaCommitmentRecord, confidential_compute::ConfidentialComputePolicy,
        types::StorageTicketId,
    },
    nexus::LaneStorageProfile,
    sorafs::pin_registry::ManifestDigest,
};
use thiserror::Error;

/// Errors raised while validating confidential-compute commitments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ConfidentialComputeError {
    /// Lane was flagged as confidential but did not declare a policy/key version.
    #[error("lane is marked confidential but missing a compute policy/key version")]
    MissingPolicy,
    /// Storage ticket must be non-zero for confidential lanes.
    #[error("confidential lane requires a non-zero storage ticket")]
    ZeroStorageTicket,
    /// Payload digest must be non-zero for confidential lanes.
    #[error("confidential lane requires a non-zero payload digest")]
    ZeroPayloadDigest,
    /// Manifest digest must be non-zero for confidential lanes.
    #[error("confidential lane requires a non-zero manifest digest")]
    ZeroManifestDigest,
    /// Confidential lanes must not use the full-replica storage profile.
    #[error("confidential lane must use split/commitment-only storage, not {0:?}")]
    InvalidStorageProfile(LaneStorageProfile),
}

/// Validate a DA commitment for confidential-compute lanes.
///
/// Returns `Ok(None)` when the lane is not marked confidential. Otherwise returns
/// the resolved policy on success or a [`ConfidentialComputeError`] on failure.
///
/// # Errors
/// Returns [`ConfidentialComputeError`] when the lane is marked confidential but
/// fails policy, storage-profile, or digest validation.
pub fn validate_confidential_compute_record(
    lane_config: &ConfigLaneConfig,
    record: &DaCommitmentRecord,
) -> Result<Option<ConfidentialComputePolicy>, ConfidentialComputeError> {
    if !lane_config.is_confidential_compute(record.lane_id) {
        return Ok(None);
    }

    let entry = lane_config
        .entry(record.lane_id)
        .ok_or(ConfidentialComputeError::MissingPolicy)?;
    let Some(policy) = lane_config.confidential_compute_policy(record.lane_id) else {
        return Err(ConfidentialComputeError::MissingPolicy);
    };

    if matches!(entry.storage_profile, LaneStorageProfile::FullReplica) {
        return Err(ConfidentialComputeError::InvalidStorageProfile(
            entry.storage_profile,
        ));
    }

    if is_zero_ticket(&record.storage_ticket) {
        return Err(ConfidentialComputeError::ZeroStorageTicket);
    }

    if record.client_blob_id.is_zero() {
        return Err(ConfidentialComputeError::ZeroPayloadDigest);
    }

    if is_zero_manifest(&record.manifest_hash) {
        return Err(ConfidentialComputeError::ZeroManifestDigest);
    }

    Ok(Some(policy.clone()))
}

fn is_zero_ticket(ticket: &StorageTicketId) -> bool {
    ticket.as_ref().iter().all(|byte| *byte == 0)
}

fn is_zero_manifest(digest: &ManifestDigest) -> bool {
    digest.as_bytes().iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use iroha_config::parameters::actual::LaneConfig as ConfigLaneConfig;
    use iroha_crypto::{Hash, Signature};
    use iroha_data_model::{
        da::{
            commitment::{DaCommitmentRecord, DaProofScheme, KzgCommitment, RetentionClass},
            types::{BlobDigest, StorageTicketId},
        },
        nexus::{
            DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneStorageProfile,
        },
        sorafs::pin_registry::ManifestDigest,
    };

    use super::*;

    fn lane_config(confidential: bool, key_version: Option<u32>) -> ConfigLaneConfig {
        let mut metadata = std::collections::BTreeMap::new();
        if confidential {
            metadata.insert("confidential_compute".to_string(), "true".to_string());
        }
        if let Some(version) = key_version {
            metadata.insert("confidential_key_version".to_string(), version.to_string());
        }
        metadata.insert(
            "confidential_mechanism".to_string(),
            "encryption".to_string(),
        );
        let catalog = LaneCatalog::new(
            nonzero_ext::nonzero!(1_u32),
            vec![ModelLaneConfig {
                id: LaneId::new(0),
                dataspace_id: DataSpaceId::GLOBAL,
                alias: "lane0".into(),
                storage: LaneStorageProfile::SplitReplica,
                metadata,
                ..ModelLaneConfig::default()
            }],
        )
        .expect("catalog");
        ConfigLaneConfig::from_catalog(&catalog)
    }

    fn record_with_ticket(ticket: [u8; 32], blob: [u8; 32]) -> DaCommitmentRecord {
        DaCommitmentRecord::new(
            LaneId::new(0),
            1,
            1,
            BlobDigest::new(blob),
            ManifestDigest::new([0xBB; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([0xCC; 32]),
            Some(KzgCommitment::new([0x11; 48])),
            None,
            RetentionClass::default(),
            StorageTicketId::new(ticket),
            Signature::from_bytes(&[0x22; 64]),
        )
    }

    #[test]
    fn passes_when_policy_and_digests_present() {
        let config = lane_config(true, Some(3));
        let record = record_with_ticket([0x22; 32], [0x33; 32]);

        let policy = validate_confidential_compute_record(&config, &record)
            .expect("validation should succeed")
            .expect("policy must be returned");
        assert_eq!(policy.key_version, 3);
    }

    #[test]
    fn rejects_missing_policy() {
        let config = lane_config(true, None);
        let record = record_with_ticket([0x22; 32], [0x33; 32]);

        let err = validate_confidential_compute_record(&config, &record)
            .expect_err("missing policy must fail");
        assert!(matches!(err, ConfidentialComputeError::MissingPolicy));
    }

    #[test]
    fn rejects_zero_ticket_or_payload() {
        let config = lane_config(true, Some(1));

        let zero_ticket = record_with_ticket([0; 32], [0x33; 32]);
        assert!(matches!(
            validate_confidential_compute_record(&config, &zero_ticket).expect_err("zero ticket"),
            ConfidentialComputeError::ZeroStorageTicket
        ));

        let zero_payload = record_with_ticket([0x22; 32], [0; 32]);
        assert!(matches!(
            validate_confidential_compute_record(&config, &zero_payload).expect_err("zero payload"),
            ConfidentialComputeError::ZeroPayloadDigest
        ));
    }
}
