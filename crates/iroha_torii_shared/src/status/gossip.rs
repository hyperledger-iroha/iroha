//! Gossip status wire records.
use iroha_schema::IntoSchema;
use norito::derive::{NoritoDeserialize, NoritoSerialize};

/// Configured caps and frame limits for transaction gossip.
#[derive(
    Clone,
    Debug,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_torii_shared::status::gossip::TxGossipCaps",
    frame = "iroha_telemetry::metrics::TxGossipCaps"
)]
pub struct TxGossipCaps {
    /// Max gossip frame size in bytes for transaction payloads.
    pub frame_cap_bytes: u64,
    /// Optional cap on public gossip targets (0 = broadcast).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub public_target_cap: Option<u64>,
    /// Optional cap on restricted gossip targets (0 = commit topology).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub restricted_target_cap: Option<u64>,
    /// Public-plane target reshuffle interval in milliseconds.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub public_target_reshuffle_ms: Option<u64>,
    /// Restricted-plane target reshuffle interval in milliseconds.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub restricted_target_reshuffle_ms: Option<u64>,
    /// Whether gossip for unknown dataspaces is dropped instead of routed via the restricted plane.
    #[norito(default)]
    pub drop_unknown_dataspace: bool,
    /// Fallback policy when restricted targets are unavailable (`drop` or `public_overlay`).
    #[norito(default)]
    pub restricted_fallback: String,
    /// Policy for restricted payloads when only the public overlay is available (`refuse` or `forward`).
    pub restricted_public_policy: String,
}
impl Default for TxGossipCaps {
    fn default() -> Self {
        Self {
            frame_cap_bytes: 0,
            public_target_cap: None,
            restricted_target_cap: None,
            public_target_reshuffle_ms: None,
            restricted_target_reshuffle_ms: None,
            drop_unknown_dataspace: false,
            restricted_fallback: "drop".to_string(),
            restricted_public_policy: "refuse".to_string(),
        }
    }
}
/// Snapshot of the most recent gossip target selection for a dataspace.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_torii_shared::status::gossip::TxGossipStatus",
    frame = "iroha_telemetry::metrics::TxGossipStatus"
)]
pub struct TxGossipStatus {
    /// Plane used for the gossip attempt (`public` or `restricted`).
    pub plane: String,
    /// Dataspace identifier.
    pub dataspace_id: u64,
    /// Human-friendly dataspace alias (if configured).
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub dataspace_alias: Option<String>,
    /// Lane ids included in the gossip batch.
    #[norito(default)]
    pub lane_ids: Vec<u32>,
    /// Number of peers targeted in the latest attempt.
    pub targets: u64,
    /// Peer ids targeted in the latest attempt.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub target_peers: Vec<String>,
    /// Outcome of the latest attempt (`sent` or `dropped`).
    #[norito(default)]
    pub outcome: String,
    /// Whether restricted fallback was considered/used for this attempt.
    #[norito(default)]
    pub fallback_used: bool,
    /// Fallback surface used (e.g., `public_overlay`) when `fallback_used` is true.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub fallback_surface: Option<String>,
    /// Drop reason when the batch was refused.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Configured target cap for this plane (0 = broadcast/unlimited).
    pub target_cap: u64,
    /// Transactions included in the encoded frame.
    pub batch_txs: u64,
    /// Encoded frame length in bytes.
    pub frame_bytes: u64,
}
/// Aggregated transaction gossip snapshot for `/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_torii_shared::status::gossip::TxGossipSnapshot",
    frame = "iroha_telemetry::metrics::TxGossipSnapshot"
)]
pub struct TxGossipSnapshot {
    /// Configured caps and frame limits.
    pub caps: TxGossipCaps,
    /// Latest target selections grouped by dataspace/plane.
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<TxGossipStatus>,
}
/// Highest DA receipt sequence observed per lane/epoch.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    IntoSchema,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_torii_shared::status::gossip::DaReceiptCursorStatus",
    frame = "iroha_telemetry::metrics::DaReceiptCursorStatus"
)]
pub struct DaReceiptCursorStatus {
    /// Numeric lane identifier.
    pub lane_id: u32,
    /// Epoch scoped to the lane.
    pub epoch: u64,
    /// Highest recorded receipt sequence for the lane/epoch.
    pub highest_sequence: u64,
}

#[cfg(test)]
mod captured_frame_identity_tests {
    #[test]
    fn observed_declared_identities() {
        crate::captured_identity_tests::assert_bidirectional::<super::DaReceiptCursorStatus>(
            "iroha_torii_shared::status::gossip::DaReceiptCursorStatus",
        );
        crate::captured_identity_tests::assert_bidirectional::<super::TxGossipCaps>(
            "iroha_torii_shared::status::gossip::TxGossipCaps",
        );
        crate::captured_identity_tests::assert_bidirectional::<super::TxGossipSnapshot>(
            "iroha_torii_shared::status::gossip::TxGossipSnapshot",
        );
        crate::captured_identity_tests::assert_bidirectional::<super::TxGossipStatus>(
            "iroha_torii_shared::status::gossip::TxGossipStatus",
        );
    }
}
