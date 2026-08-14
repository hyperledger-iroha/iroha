impl State {
    /// Select only certificates eligible for ordinary canonical-body receipt
    /// reconstruction. READY identifies autonomous economic execution, whose
    /// receipt must instead come from its exact globally finalized merge carrier.
    fn ordinary_application_receipt_repair_session(
        artifact: crate::kura::CertifiedLaneBlockArtifact,
    ) -> Option<crate::lane_consensus::CommittedLaneBlockSession> {
        let session = crate::lane_consensus::CommittedLaneBlockSession {
            proposal: artifact.proposal,
            prepare_qc: artifact.prepare_qc,
            commit_qc: artifact.commit_qc,
        };
        session
            .prepare_qc
            .payload_availability_qc
            .is_none()
            .then_some(session)
    }
    /// Check the predecessor according to the lane certificate's execution role.
    /// READY-bearing autonomous certificates may advance only from globally
    /// applied merge evidence; ordinary/control certificates retain their
    /// hash-only snapshot compatibility.
    pub(crate) fn certified_lane_block_session_predecessor_is_applied_cached(
        &self,
        session: &crate::lane_consensus::CommittedLaneBlockSession,
    ) -> bool {
        if session.prepare_qc.payload_availability_qc.is_some() {
            self.certified_autonomous_lane_block_predecessor_is_globally_applied_cached(
                &session.proposal,
            )
        } else {
            self.certified_lane_block_predecessor_is_applied_or_snapshot_anchored_cached(
                &session.proposal,
            )
        }
    }
    /// Require canonical economic application of an autonomous proposal.
    ///
    /// A lane ownership artifact proves payload routing, not WSV application.
    /// Autonomous application is therefore admitted only by an exact
    /// merge-log/carrier receipt or the replicated per-incarnation WSV frontier.
    /// Malformed replicated marker bytes fail closed even if local Kura happens
    /// to contain a receipt.
    fn certified_autonomous_lane_block_is_globally_applied_cached(
        &self,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> bool {
        let descriptor = &proposal.descriptor;
        if descriptor.lane_block_height == 0 {
            return false;
        }
        let world = self.world.view();
        let Ok(frontier) = Self::canonical_merged_lane_frontier_from_world(
            &world,
            descriptor.lane_id,
            descriptor.dataspace_id,
            descriptor.lane_incarnation,
        ) else {
            return false;
        };
        frontier
            == (
                descriptor.lane_block_height,
                Some(descriptor.descriptor_hash),
            )
            || self
                .kura
                .autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair(proposal)
    }
    /// Require canonical economic application of the exact predecessor of an
    /// autonomous proposal.
    ///
    /// Unlike the ordinary helper, this deliberately ignores hash-only lane
    /// ownership snapshots. Those snapshots authenticate a canonical carrier
    /// identity but cannot prove that its autonomous effects crossed the WSV
    /// application boundary.
    pub(crate) fn certified_autonomous_lane_block_predecessor_is_globally_applied_cached(
        &self,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> bool {
        let descriptor = &proposal.descriptor;
        let previous_height = descriptor.previous_lane_block_height;
        if previous_height == 0 {
            return descriptor.lane_block_height == 1
                && descriptor.previous_lane_block_descriptor_hash.is_none();
        }
        let Some(previous_descriptor_hash) = descriptor.previous_lane_block_descriptor_hash else {
            return false;
        };
        if previous_height.checked_add(1) != Some(descriptor.lane_block_height) {
            return false;
        }
        let world = self.world.view();
        let Ok(frontier) = Self::canonical_merged_lane_frontier_from_world(
            &world,
            descriptor.lane_id,
            descriptor.dataspace_id,
            descriptor.lane_incarnation,
        ) else {
            return false;
        };
        if frontier.0 >= previous_height {
            return frontier == (previous_height, Some(previous_descriptor_hash));
        }
        self.kura
            .autonomous_lane_block_predecessor_merge_receipt_revalidates_without_sidecar_repair(
                proposal,
            )
    }
}
