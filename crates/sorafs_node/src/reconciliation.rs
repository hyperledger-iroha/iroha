//! Deterministic reconciliation snapshots for repair and GC state.

use blake3::hash;
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::repair::RepairTaskStateV1;
use sorafs_manifest::retention::RetentionSourceV1;

use crate::store::ChunkRefcountEntry;

pub(crate) const RECONCILIATION_SNAPSHOT_VERSION_V1: u8 = 1;

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct RepairReconciliationSnapshot {
    pub(crate) version: u8,
    pub(crate) tasks: Vec<RepairReconciliationEntry>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct RepairReconciliationEntry {
    pub(crate) ticket_id: String,
    pub(crate) manifest_digest: [u8; 32],
    pub(crate) provider_id: [u8; 32],
    pub(crate) state: RepairTaskStateV1,
}

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

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct GcReconciliationSnapshot {
    pub(crate) version: u8,
    pub(crate) gc_freed_bytes_total: u64,
    pub(crate) gc_evictions_total: u64,
    #[norito(default)]
    pub(crate) chunk_refcounts: Vec<ChunkRefcountEntry>,
}

pub(crate) fn hash_snapshot<T: norito::core::NoritoSerialize>(
    snapshot: &T,
) -> Result<[u8; 32], norito::Error> {
    let bytes = norito::to_bytes(snapshot)?;
    Ok(*hash(&bytes).as_bytes())
}
