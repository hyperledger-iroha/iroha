//! Deterministic reconciliation snapshots for repair and GC state.
use crate::store::ChunkRefcountEntry;
use blake3::hash;
use iroha_data_model::sorafs::moderation_ledger::{RepairFinalizedCursorV1, RepairLedgerTaskV1};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::deal::XorQuantity;
use sorafs_manifest::retention::RetentionSourceV1;
pub(crate) const RECONCILIATION_SNAPSHOT_VERSION_V1: u8 = 1;
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::reconciliation::RepairReconciliationSnapshot")]
pub(crate) struct RepairReconciliationSnapshot {
    pub(crate) version: u8,
    pub(crate) finalized_cursor: RepairFinalizedCursorV1,
    pub(crate) tasks: Vec<RepairLedgerTaskV1>,
}
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::reconciliation::RetentionReconciliationSnapshot")]
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
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::reconciliation::GcReconciliationSnapshot")]
pub(crate) struct GcReconciliationSnapshot {
    pub(crate) version: u8,
    pub(crate) gc_freed_bytes_total: u64,
    pub(crate) gc_evictions_total: u64,
    #[norito(default)]
    pub(crate) chunk_refcounts: Vec<ChunkRefcountEntry>,
}
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::reconciliation::AppealFinanceRollupReconciliationSnapshot")]
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

    fn assert_snapshot_frame<T>(value: &T, owner: &str) -> [u8; 32]
    where
        T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
    {
        assert_eq!(T::nominal_name(), owner);
        assert_eq!(T::frame_name(), owner);
        let encoded = norito::encode_canonical(value).expect("snapshot frame");
        assert_eq!(encoded[6..22], norito::schema::identity::frame_hash::<T>());
        let decoded: T = norito::decode_canonical(&encoded).expect("snapshot roundtrip");
        assert_eq!(
            norito::codec::encode_adaptive(&decoded),
            norito::codec::encode_adaptive(value)
        );
        let digest = hash_snapshot(value).expect("actual reconciliation digest");
        assert_eq!(digest, *hash(&encoded).as_bytes());
        let mut wrong_owner = encoded;
        wrong_owner[6] ^= 1;
        assert!(matches!(
            norito::decode_canonical::<T>(&wrong_owner),
            Err(norito::Error::SchemaMismatch)
        ));
        digest
    }

    #[test]
    fn reconciliation_frames_bind_each_snapshot_owner_and_payload() {
        let repair = RepairReconciliationSnapshot {
            version: RECONCILIATION_SNAPSHOT_VERSION_V1,
            finalized_cursor: RepairFinalizedCursorV1 {
                height: 7,
                block_hash: [0x31; 32],
            },
            tasks: Vec::new(),
        };
        let mut retention = RetentionReconciliationSnapshot {
            version: RECONCILIATION_SNAPSHOT_VERSION_V1,
            manifests: vec![RetentionReconciliationEntry {
                manifest_id: "retained".to_owned(),
                manifest_digest: [0x42; 32],
                retention_epoch: 1234,
                retention_source: None,
            }],
        };
        let gc = GcReconciliationSnapshot {
            version: RECONCILIATION_SNAPSHOT_VERSION_V1,
            gc_freed_bytes_total: 456,
            gc_evictions_total: 2,
            chunk_refcounts: vec![ChunkRefcountEntry {
                digest: [0x53; 32],
                count: 3,
            }],
        };
        let appeal = AppealFinanceRollupReconciliationSnapshot {
            version: RECONCILIATION_SNAPSHOT_VERSION_V1,
            rollups: vec![AppealFinanceRollupReconciliationEntry {
                cycle: "2026-09".to_owned(),
                encoded_blake3: "64".repeat(32),
                report_count: 5,
                case_count: 6,
                total_treasury_xor: XorQuantity::zero(),
                total_rewards_forfeited_treasury_xor: XorQuantity::zero(),
                generated_at_unix_ms: 9876,
            }],
        };
        let digests = [
            assert_snapshot_frame(
                &repair,
                "sorafs_node::reconciliation::RepairReconciliationSnapshot",
            ),
            assert_snapshot_frame(
                &retention,
                "sorafs_node::reconciliation::RetentionReconciliationSnapshot",
            ),
            assert_snapshot_frame(&gc, "sorafs_node::reconciliation::GcReconciliationSnapshot"),
            assert_snapshot_frame(
                &appeal,
                "sorafs_node::reconciliation::AppealFinanceRollupReconciliationSnapshot",
            ),
        ];
        assert_eq!(
            digests
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            4
        );
        retention.manifests[0].retention_epoch += 1;
        assert_ne!(hash_snapshot(&retention).unwrap(), digests[1]);
    }
}
