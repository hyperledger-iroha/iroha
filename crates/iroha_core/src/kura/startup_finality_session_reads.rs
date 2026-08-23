impl V2StartupFinalityVerificationSession<'_> {
    /// Load one canonical body through this session's exact Kura owner.
    ///
    /// Hash-only audited snapshot heights are the only legitimate absence.
    /// Any missing or conflicting executable body is corruption, not pending
    /// lane completion.
    pub(crate) fn canonical_block(&self, height: NonZeroUsize) -> Result<Option<Arc<SignedBlock>>> {
        let height_u64 = u64::try_from(height.get())?;
        if self.is_hash_only_height(height_u64) {
            return Ok(None);
        }
        let expected_hash = self.canonical_hash(height_u64).ok_or_else(|| {
            Kura::invalid_lane_artifact_error(
                self.kura.store_root.clone(),
                "startup lane completion height is outside the verified replay boundary",
            )
        })?;
        let block = self
            .kura
            .get_block_without_merge_sidecar(height)
            .ok_or_else(|| {
                Kura::invalid_lane_artifact_error(
                    self.kura.store_root.clone(),
                    "startup lane completion has no readable canonical block body",
                )
            })?;
        if block.header().height().get() != height_u64 || block.hash() != expected_hash {
            return Err(Kura::invalid_lane_artifact_error(
                self.kura.store_root.clone(),
                "startup lane completion block differs from the verified replay boundary",
            ));
        }
        Ok(Some(block))
    }
    /// Read an exact certified lane slot without repairing any sidecar.
    pub(crate) fn certified_lane_block_artifact(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> Result<Option<CertifiedLaneBlockArtifact>> {
        let descriptor = &proposal.descriptor;
        let artifact = self
            .kura
            .read_certified_lane_block_artifact_read_only_under_prune_and_canonical_guards(
                descriptor.lane_id,
                descriptor.lane_block_height,
            )?;
        if artifact
            .as_ref()
            .is_some_and(|artifact| artifact.proposal != *proposal)
        {
            return Err(Kura::invalid_lane_artifact_error(
                self.kura.store_root.clone(),
                "startup certified lane slot conflicts with the finalized proposal",
            ));
        }
        Ok(artifact)
    }
    /// Read an exact application receipt without repairing any sidecar.
    pub(crate) fn lane_block_application_receipt(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> Result<Option<LaneBlockApplicationReceiptArtifact>> {
        Ok(self
            .kura
            .read_exact_lane_block_application_receipt_under_prune_and_canonical_guards(proposal))
    }
    /// Read an exact autonomous payload without promoting a view-state temp.
    pub(crate) fn autonomous_lane_block_artifact(
        &self,
        proposal: &LaneBlockProposalV1,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
    ) -> Result<Option<AutonomousLaneBlockArtifact>> {
        let descriptor = &proposal.descriptor;
        let artifact = self
            .kura
            .read_autonomous_lane_block_artifact_with_recovery_policy(
                descriptor.lane_id,
                descriptor.lane_block_height,
                expected_network_id,
                expected_epoch,
                false,
            );
        if artifact
            .as_ref()
            .is_some_and(|artifact| artifact.executable_payload.origin_proposal != *proposal)
        {
            return Err(Kura::invalid_lane_artifact_error(
                self.kura.store_root.clone(),
                "startup autonomous lane slot conflicts with the finalized proposal",
            ));
        }
        Ok(artifact)
    }
}
