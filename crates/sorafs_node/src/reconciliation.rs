//! Deterministic reconciliation snapshots for repair and GC state.
use crate::store::ChunkRefcountEntry;
use blake3::hash;
use iroha_data_model::sorafs::moderation_ledger::{RepairFinalizedCursorV1, RepairLedgerTaskV1};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::deal::XorQuantity;
use sorafs_manifest::retention::RetentionSourceV1;
pub(crate) const RECONCILIATION_SNAPSHOT_VERSION_V1: u8 = 1;
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::reconciliation::RepairReconciliationSnapshot")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct RepairReconciliationSnapshot {
    pub(crate) version: u8,
    pub(crate) finalized_cursor: RepairFinalizedCursorV1,
    pub(crate) tasks: Vec<RepairLedgerTaskV1>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::reconciliation::RetentionReconciliationSnapshot")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct RetentionReconciliationSnapshot {
    pub(crate) version: u8,
    pub(crate) manifests: Vec<RetentionReconciliationEntry>,
}
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct RetentionReconciliationEntry {
    pub(crate) manifest_id: String,
    pub(crate) manifest_digest: [u8; 32],
    pub(crate) retention_epoch: u64,
    #[norito(default)]
    pub(crate) retention_source: Option<RetentionSourceV1>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::reconciliation::GcReconciliationSnapshot")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct GcReconciliationSnapshot {
    pub(crate) version: u8,
    pub(crate) gc_freed_bytes_total: u64,
    pub(crate) gc_evictions_total: u64,
    #[norito(default)]
    pub(crate) chunk_refcounts: Vec<ChunkRefcountEntry>,
}
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::reconciliation::AppealFinanceRollupReconciliationSnapshot")]
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct AppealFinanceRollupReconciliationSnapshot {
    pub(crate) version: u8,
    pub(crate) rollups: Vec<AppealFinanceRollupReconciliationEntry>,
}
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct AppealFinanceRollupReconciliationEntry {
    pub(crate) cycle: String,
    pub(crate) encoded_blake3: String,
    pub(crate) report_count: u64,
    pub(crate) case_count: u64,
    pub(crate) total_treasury_xor: XorQuantity,
    pub(crate) total_rewards_forfeited_treasury_xor: XorQuantity,
    pub(crate) generated_at_unix_ms: u64,
}
pub(crate) fn hash_snapshot<T: norito::core::NoritoSerialize>(
    snapshot: &T,
) -> Result<[u8; 32], norito::Error> {
    let bytes = norito::to_bytes(snapshot)?;
    Ok(*hash(&bytes).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconciliation_snapshot_frames_separate_domains_and_bind_state_changes() {
        use crate::schema_identity_test_support::assert_canonical_frame;
        let repair = RepairReconciliationSnapshot {
            version: RECONCILIATION_SNAPSHOT_VERSION_V1,
            finalized_cursor: RepairFinalizedCursorV1 {
                height: 7,
                block_hash: [0xA1; 32],
            },
            tasks: Vec::new(),
        };
        let retention = RetentionReconciliationSnapshot {
            version: RECONCILIATION_SNAPSHOT_VERSION_V1,
            manifests: vec![RetentionReconciliationEntry {
                manifest_id: "manifest".to_owned(),
                manifest_digest: [0xA2; 32],
                retention_epoch: 9,
                retention_source: None,
            }],
        };
        let gc = GcReconciliationSnapshot {
            version: RECONCILIATION_SNAPSHOT_VERSION_V1,
            gc_freed_bytes_total: 256,
            gc_evictions_total: 2,
            chunk_refcounts: vec![ChunkRefcountEntry {
                digest: [0xA3; 32],
                count: 3,
            }],
        };
        let finance = AppealFinanceRollupReconciliationSnapshot {
            version: RECONCILIATION_SNAPSHOT_VERSION_V1,
            rollups: Vec::new(),
        };
        let repair_bytes = assert_canonical_frame(
            &repair,
            "sorafs_node::reconciliation::RepairReconciliationSnapshot",
        );
        let retention_bytes = assert_canonical_frame(
            &retention,
            "sorafs_node::reconciliation::RetentionReconciliationSnapshot",
        );
        let gc_bytes =
            assert_canonical_frame(&gc, "sorafs_node::reconciliation::GcReconciliationSnapshot");
        let finance_bytes = assert_canonical_frame(
            &finance,
            "sorafs_node::reconciliation::AppealFinanceRollupReconciliationSnapshot",
        );
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        assert_eq!(
            hash_snapshot(&repair).unwrap(),
            *blake3::hash(&repair_bytes).as_bytes()
        );
        assert_eq!(
            hash_snapshot(&retention).unwrap(),
            *blake3::hash(&retention_bytes).as_bytes()
        );
        assert_eq!(
            hash_snapshot(&gc).unwrap(),
            *blake3::hash(&gc_bytes).as_bytes()
        );
        assert_eq!(
            hash_snapshot(&finance).unwrap(),
            *blake3::hash(&finance_bytes).as_bytes()
        );
        assert!(matches!(
            norito::decode_canonical::<AppealFinanceRollupReconciliationSnapshot>(&repair_bytes),
            Err(norito::Error::SchemaMismatch)
        ));
        assert!(matches!(
            norito::decode_canonical::<GcReconciliationSnapshot>(&retention_bytes),
            Err(norito::Error::SchemaMismatch)
        ));
        let mut changed = gc.clone();
        changed.chunk_refcounts[0].count += 1;
        assert_ne!(
            hash_snapshot(&changed).unwrap(),
            hash_snapshot(&gc).unwrap()
        );
    }
}
