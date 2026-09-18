//! Read-only fee effect expectations derived from committed execution transcripts.

use super::{MergeExecutionBatch, MergeLedgerCommitError, State, StatePath};

/// Derive the complete canonical fee marker set without consulting live fee policy.
/// An empty receipt transcript has no fee effects; every recorded receipt requires
/// its source marker and its enclosing settlement marker, including a zero amount.
pub(super) fn expected(
    batch: &MergeExecutionBatch,
) -> Result<Vec<(StatePath, Vec<u8>)>, MergeLedgerCommitError> {
    let mut markers = Vec::new();
    for execution in &batch.lanes {
        let commitment = &execution.settlement_commitment;
        let settlement_hash = execution.settlement_hash;
        if commitment.nexus_fee_receipts.is_empty() {
            continue;
        }
        markers.push((
            State::nexus_fee_settlement_marker_key(
                commitment.dataspace_id,
                commitment.lane_id,
                commitment.block_height,
                &settlement_hash,
            )?,
            vec![1],
        ));
        for receipt in &commitment.nexus_fee_receipts {
            markers.push((
                State::nexus_fee_receipt_marker_key(&receipt.source_id)?,
                vec![1],
            ));
        }
    }
    Ok(markers)
}
