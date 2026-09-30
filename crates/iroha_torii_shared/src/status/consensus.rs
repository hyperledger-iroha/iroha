//! Consensus status wire records.
use iroha_schema::IntoSchema;
use norito::derive::{NoritoDeserialize, NoritoSerialize};

/// Live node-wide consensus observations exposed via `/status`.
/// Per-instance round, leader, certificate and progress state lives at `/v1/sumeragi/status`.
#[derive(
    Clone,
    Debug,
    Default,
    IntoSchema,
    NoritoSerialize,
    NoritoDeserialize,
    norito::derive::JsonSerialize,
    norito::derive::JsonDeserialize,
)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "first-release consensus telemetry exposes independent status flags without compatibility aliases"
)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii_shared::status::consensus::SumeragiConsensusStatus")]
#[norito(deny_unknown_fields)]
pub struct SumeragiConsensusStatus {
    /// Current runtime consensus mode tag.
    pub mode_tag: String,
    /// Current number of transactions observed in the local queue.
    pub tx_queue_depth: u64,
    /// Configured queue capacity on this peer.
    pub tx_queue_capacity: u64,
    /// Estimated retained queue bytes on this peer.
    pub tx_queue_retained_bytes: u64,
    /// Configured retained queue byte budget on this peer.
    pub tx_queue_max_retained_bytes: u64,
    /// Whether the local transaction queue is saturated.
    pub tx_queue_saturated: bool,
    /// Whether the local transaction queue is saturated by transaction count.
    pub tx_queue_saturated_by_count: bool,
    /// Whether the local transaction queue is saturated by retained bytes.
    pub tx_queue_saturated_by_bytes: bool,
    /// Whether the local transaction queue is saturated by oldest queued age.
    pub tx_queue_saturated_by_age: bool,
    /// Oldest queued transaction age in milliseconds.
    pub tx_queue_oldest_queued_age_ms: u64,
    /// Total lanes that remain sealed awaiting governance manifests.
    pub lane_governance_sealed_total: u32,
    /// Aliases of lanes that remain sealed awaiting governance manifests.
    pub lane_governance_sealed_aliases: Vec<String>,
}

#[cfg(test)]
mod captured_frame_identity_tests {
    #[test]
    fn observed_declared_identities() {
        crate::captured_identity_tests::assert_bidirectional::<super::SumeragiConsensusStatus>(
            "iroha_torii_shared::status::consensus::SumeragiConsensusStatus",
        );
    }
}

#[cfg(test)]
mod live_status_contract_tests {
    use super::SumeragiConsensusStatus;
    use norito::json::{self, Value};

    #[test]
    fn retired_consensus_fields_are_rejected_instead_of_silently_discarded() {
        let current = json::to_value(&SumeragiConsensusStatus::default()).unwrap();
        for name in [
            "leader_index",
            "highest_qc_height",
            "locked_qc_height",
            "locked_qc_view",
            "commit_signatures_present",
            "commit_signatures_counted",
            "commit_signatures_set_b",
            "commit_signatures_required",
            "commit_qc_height",
            "commit_qc_view",
            "commit_qc_epoch",
            "commit_qc_signatures_total",
            "commit_qc_validator_set_len",
            "block_created_dropped_by_lock_total",
            "block_created_hint_mismatch_total",
            "block_created_proposal_mismatch_total",
            "epoch_length_blocks",
            "epoch_commit_deadline_offset",
            "epoch_reveal_deadline_offset",
            "prf_epoch_seed",
            "prf_height",
            "prf_view",
            "view_change_proof_accepted_total",
            "view_change_proof_stale_total",
            "view_change_proof_rejected_total",
            "view_change_suggest_total",
            "view_change_install_total",
        ] {
            let mut stale = current.clone();
            stale
                .as_object_mut()
                .unwrap()
                .insert(name.to_owned(), Value::from(0_u64));
            assert!(
                json::from_value::<SumeragiConsensusStatus>(stale).is_err(),
                "accepted {name}"
            );
        }
    }

    #[test]
    fn every_live_observation_is_required_on_the_wire() {
        let current = json::to_value(&SumeragiConsensusStatus::default()).unwrap();
        let fields = current.as_object().unwrap();
        assert_eq!(fields.len(), 12);
        for name in fields.keys() {
            let mut incomplete = current.clone();
            incomplete.as_object_mut().unwrap().remove(name);
            assert!(
                json::from_value::<SumeragiConsensusStatus>(incomplete).is_err(),
                "defaulted {name}"
            );
        }
    }
}
