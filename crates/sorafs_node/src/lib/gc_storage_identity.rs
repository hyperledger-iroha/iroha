#[derive(norito::NoritoSchema)]
#[norito_schema(name = "sorafs_node::GcStorageIdentityV1")]
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct GcStorageIdentityV1 {
    total_bytes: u64,
    manifest_count: u64,
    gc_freed_bytes_total: u64,
    gc_evictions_total: u64,
    manifest_set_digest: [u8; 32],
    chunk_refcounts_digest: [u8; 32],
}
