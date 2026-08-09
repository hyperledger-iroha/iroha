impl Kura {
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
            && self.lane_block_application_receipt_matches_merge_log(&receipt)
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
            && self.lane_block_application_receipt_matches_merge_log(&receipt)
    }
}
