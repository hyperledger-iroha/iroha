//! Taikai status wire records.
use iroha_schema::IntoSchema;

/// Snapshot of Taikai ingest health per (cluster, stream) surfaced via `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_torii_shared::status::taikai::TaikaiIngestStatus",
    frame = "iroha_telemetry::metrics::TaikaiIngestStatus"
)]
pub struct TaikaiIngestStatus {
    /// Cluster label associated with the ingest pipeline.
    pub cluster: String,
    /// Stream identifier within the cluster.
    pub stream: String,
    /// Last observed encoder-to-ingest latency in milliseconds.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_latency_ms: Option<u32>,
    /// Last observed signed live-edge drift in milliseconds (negative = ahead).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_live_edge_drift_ms: Option<i32>,
    /// Aggregated ingest error counters grouped by reason.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub error_counts: Vec<TaikaiIngestErrorCounter>,
}
/// Aggregated error counter for a given reason.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_torii_shared::status::taikai::TaikaiIngestErrorCounter",
    frame = "iroha_telemetry::metrics::TaikaiIngestErrorCounter"
)]
pub struct TaikaiIngestErrorCounter {
    /// Normalised error reason identifier (HTTP canonical reason or status code).
    pub reason: String,
    /// Total occurrences observed by the node.
    pub total: u64,
}
/// Snapshot of alias rotation events coming from Taikai routing manifests.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_torii_shared::status::taikai::TaikaiAliasRotationStatus",
    frame = "iroha_telemetry::metrics::TaikaiAliasRotationStatus"
)]
pub struct TaikaiAliasRotationStatus {
    /// Cluster label associated with the ingest pipeline.
    pub cluster: String,
    /// Event identifier.
    pub event: String,
    /// Stream identifier.
    pub stream: String,
    /// Namespace portion of the alias binding (e.g., `sora`).
    pub alias_namespace: String,
    /// Alias label bound to the TRM (e.g., `docs`).
    pub alias_name: String,
    /// Inclusive start of the manifest coverage window.
    pub window_start_sequence: u64,
    /// Inclusive end of the manifest coverage window.
    pub window_end_sequence: u64,
    /// Hex-encoded digest of the accepted routing manifest.
    pub manifest_digest_hex: String,
    /// Total rotations observed for this stream/event pair.
    pub rotations_total: u64,
    /// UNIX timestamp (seconds) when this snapshot was last updated.
    pub last_updated_unix: u64,
}

#[cfg(test)]
mod captured_frame_identity_tests {
    #[test]
    fn observed_declared_identities() {
        crate::captured_identity_tests::assert_bidirectional::<super::TaikaiAliasRotationStatus>(
            "iroha_torii_shared::status::taikai::TaikaiAliasRotationStatus",
        );
        crate::captured_identity_tests::assert_bidirectional::<super::TaikaiIngestErrorCounter>(
            "iroha_torii_shared::status::taikai::TaikaiIngestErrorCounter",
        );
        crate::captured_identity_tests::assert_bidirectional::<super::TaikaiIngestStatus>(
            "iroha_torii_shared::status::taikai::TaikaiIngestStatus",
        );
    }
}
