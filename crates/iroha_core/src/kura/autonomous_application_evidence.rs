impl Kura {
    /// Revalidate the exact canonical-block receipt for the predecessor of an
    /// autonomous proposal without repairing any sidecars.
    ///
    /// Ordinary lane work can be the globally applied predecessor of later
    /// autonomous work. Its application proof is a `Current` receipt rather
    /// than a merge receipt, so it must be admitted explicitly without also
    /// admitting hash-only snapshots or direct-execution receipts.
    pub(crate) fn canonical_lane_block_predecessor_receipt_revalidates_without_sidecar_repair(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> bool {
        let descriptor = &proposal.descriptor;
        let previous_height = descriptor.previous_lane_block_height;
        let Some(previous_descriptor_hash) = descriptor.previous_lane_block_descriptor_hash else {
            return false;
        };
        if previous_height == 0
            || previous_height.checked_add(1) != Some(descriptor.lane_block_height)
        {
            return false;
        }

        // Hold the outer pruning fence while the candidate receipt is read and
        // then revalidated against the canonical block/results corridor. The
        // exact read acquires `canonical_chain_lock` before geometry/sidecars,
        // preserving Kura's established lock order.
        let _prune_guard = self.prune_lock.lock();
        let Some(receipt) = self.read_active_lane_block_application_receipt_structural(
            descriptor.lane_id,
            previous_height,
            false,
        ) else {
            return false;
        };
        let previous = &receipt.proposal.descriptor;
        if receipt.format != LaneBlockApplicationReceiptArtifactFormat::Current
            || previous.lane_id != descriptor.lane_id
            || previous.dataspace_id != descriptor.dataspace_id
            || previous.lane_incarnation != descriptor.lane_incarnation
            || previous.lane_block_height != previous_height
            || previous.descriptor_hash != previous_descriptor_hash
            || previous.proposal_height >= descriptor.proposal_height
        {
            return false;
        }
        self.read_exact_lane_block_application_receipt_under_prune_guard(&receipt.proposal)
            .as_ref()
            == Some(&receipt)
    }

    /// Revalidate one autonomous application receipt against its exact merge
    /// entry and sparse canonical carrier without repairing sidecars.
    ///
    /// A structurally valid `MergeExecution` receipt is not sufficient for
    /// autonomous successor admission: the referenced execution batch and carrier
    /// must still be present and must reconstruct these exact bytes.
    pub(crate) fn autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> bool {
        let descriptor = &proposal.descriptor;
        let Some(receipt) = self.read_lane_block_application_receipt_without_sidecar_repair(
            descriptor.lane_id,
            descriptor.lane_block_height,
        ) else {
            return false;
        };
        receipt.format == LaneBlockApplicationReceiptArtifactFormat::MergeExecution
            && receipt.proposal == *proposal
            && self
                .lane_block_application_receipt_matches_merge_log_without_sidecar_repair(&receipt)
    }
    /// Revalidate the exact predecessor of an autonomous proposal against a merge
    /// receipt and its authenticated merge-log carrier.
    pub(crate) fn autonomous_lane_block_predecessor_merge_receipt_revalidates_without_sidecar_repair(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> bool {
        let descriptor = &proposal.descriptor;
        let previous_height = descriptor.previous_lane_block_height;
        let Some(previous_descriptor_hash) = descriptor.previous_lane_block_descriptor_hash else {
            return false;
        };
        if previous_height == 0
            || previous_height.checked_add(1) != Some(descriptor.lane_block_height)
        {
            return false;
        }
        let Some(receipt) = self.read_lane_block_application_receipt_without_sidecar_repair(
            descriptor.lane_id,
            previous_height,
        ) else {
            return false;
        };
        let previous = &receipt.proposal.descriptor;
        receipt.format == LaneBlockApplicationReceiptArtifactFormat::MergeExecution
            && previous.lane_id == descriptor.lane_id
            && previous.dataspace_id == descriptor.dataspace_id
            && previous.lane_incarnation == descriptor.lane_incarnation
            && previous.lane_block_height == previous_height
            && previous.descriptor_hash == previous_descriptor_hash
            && previous.proposal_height < descriptor.proposal_height
            && self
                .lane_block_application_receipt_matches_merge_log_without_sidecar_repair(&receipt)
    }
}
include!("passive_diagnostic_reads.rs");
