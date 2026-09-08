//! Consensus status wire records.
use iroha_schema::IntoSchema;
use norito::derive::{NoritoDeserialize, NoritoSerialize};

/// Snapshot of core consensus state exposed via `/status`.
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
#[norito_schema(
    name = "iroha_torii_shared::status::consensus::SumeragiConsensusStatus",
    frame = "iroha_telemetry::metrics::SumeragiConsensusStatus"
)]
pub struct SumeragiConsensusStatus {
    /// Current runtime consensus mode tag.
    pub mode_tag: String,
    /// Current leader index (topology position).
    pub leader_index: u64,
    /// HighestQC height.
    pub highest_qc_height: u64,
    /// LockedQC height.
    pub locked_qc_height: u64,
    /// LockedQC view.
    pub locked_qc_view: u64,
    /// Signatures present on the most recently committed block.
    pub commit_signatures_present: u64,
    /// Signatures counted toward the commit quorum.
    pub commit_signatures_counted: u64,
    /// Signatures contributed by set-B validators.
    pub commit_signatures_set_b: u64,
    /// Required commit quorum size for the active topology.
    pub commit_signatures_required: u64,
    /// Latest commit certificate height (best-effort).
    pub commit_qc_height: u64,
    /// Latest commit certificate view (best-effort).
    pub commit_qc_view: u64,
    /// Latest commit certificate epoch (best-effort).
    pub commit_qc_epoch: u64,
    /// Signatures attached to the latest commit certificate.
    pub commit_qc_signatures_total: u64,
    /// Validator-set size for the latest commit certificate.
    pub commit_qc_validator_set_len: u64,
    /// Total BlockCreated drops due to locked QC gate.
    pub block_created_dropped_by_lock_total: u64,
    /// Total BlockCreated drops due to hint mismatches.
    pub block_created_hint_mismatch_total: u64,
    /// Total BlockCreated drops due to proposal mismatches.
    pub block_created_proposal_mismatch_total: u64,
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
    /// Epoch length in blocks (NPoS mode; zero when not applicable).
    pub epoch_length_blocks: u64,
    /// Commit window deadline offset from epoch start (blocks; zero when not applicable).
    pub epoch_commit_deadline_offset: u64,
    /// Reveal window deadline offset from epoch start (blocks; zero when not applicable).
    pub epoch_reveal_deadline_offset: u64,
    /// PRF epoch seed (hex) used for deterministic leader/collector selection (NPoS mode).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub prf_epoch_seed: Option<String>,
    /// Height associated with the recorded PRF context.
    pub prf_height: u64,
    /// View associated with the recorded PRF context.
    pub prf_view: u64,
    /// Total view-change proofs accepted (advanced the proof chain).
    pub view_change_proof_accepted_total: u64,
    /// Total view-change proofs ignored as stale/outdated.
    pub view_change_proof_stale_total: u64,
    /// Total view-change proofs rejected as invalid.
    pub view_change_proof_rejected_total: u64,
    /// Total view-change suggestions emitted locally.
    pub view_change_suggest_total: u64,
    /// Total installed view changes (proof advanced locally).
    pub view_change_install_total: u64,
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
