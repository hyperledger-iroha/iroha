//! Reconciliation reports published by SoraFS nodes.

use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};
use thiserror::Error;

/// Schema version for [`SorafsReconciliationReportV1`].
pub const SORAFS_RECONCILIATION_REPORT_VERSION_V1: u8 = 1;

/// Deterministic reconciliation summary published by a SoraFS node.
#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize,
)]
pub struct SorafsReconciliationReportV1 {
    /// Schema version (`SORAFS_RECONCILIATION_REPORT_VERSION_V1`).
    pub version: u8,
    /// Provider identifier submitting the report.
    pub provider_id: [u8; 32],
    /// Unix timestamp (seconds) when the report was generated.
    pub generated_at_unix: u64,
    /// BLAKE3 digest of the repair-ticket snapshot.
    pub repair_snapshot_hash: [u8; 32],
    /// BLAKE3 digest of the retention index snapshot.
    pub retention_snapshot_hash: [u8; 32],
    /// BLAKE3 digest of the GC counters/refcounts snapshot.
    pub gc_snapshot_hash: [u8; 32],
    /// Total number of repair tickets included in the snapshot.
    pub repair_task_count: u32,
    /// Total number of manifests included in the retention snapshot.
    pub retention_manifest_count: u32,
    /// Lifetime GC eviction count as reported by the local store.
    pub gc_evictions_total: u64,
    /// Lifetime GC freed-byte counter as reported by the local store.
    pub gc_freed_bytes_total: u64,
    /// Count of detected inconsistencies in the local reconciliation run.
    pub divergence_count: u32,
}

impl SorafsReconciliationReportV1 {
    /// Validate the report contents.
    pub fn validate(&self) -> Result<(), ReconciliationValidationError> {
        if self.version != SORAFS_RECONCILIATION_REPORT_VERSION_V1 {
            return Err(ReconciliationValidationError::UnsupportedVersion {
                version: self.version,
            });
        }
        if self.generated_at_unix == 0 {
            return Err(ReconciliationValidationError::InvalidTimestamp);
        }
        Ok(())
    }
}

/// Validation errors for reconciliation reports.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReconciliationValidationError {
    /// Report version not supported.
    #[error("unsupported reconciliation report version {version}")]
    UnsupportedVersion { version: u8 },
    /// Timestamp missing or invalid.
    #[error("reconciliation report timestamp must be non-zero")]
    InvalidTimestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconciliation_report_validates() {
        let report = SorafsReconciliationReportV1 {
            version: SORAFS_RECONCILIATION_REPORT_VERSION_V1,
            provider_id: [0x11; 32],
            generated_at_unix: 1_700_000_123,
            repair_snapshot_hash: [0x01; 32],
            retention_snapshot_hash: [0x02; 32],
            gc_snapshot_hash: [0x03; 32],
            repair_task_count: 2,
            retention_manifest_count: 3,
            gc_evictions_total: 4,
            gc_freed_bytes_total: 5,
            divergence_count: 0,
        };
        report.validate().expect("report should validate");
    }
}
