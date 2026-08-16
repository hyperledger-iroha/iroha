/// Fully revalidated terminal source for one exact autonomous production attempt.
///
/// The artifact and its current synthetic cursor come from the immutable
/// proposal-height attempt namespace. The retirement is accepted only when it
/// belongs to that exact payload.
pub(crate) struct AutonomousLaneRetiredAttempt {
    /// Durable producer-authenticated payload and contiguous NewView proof suffix.
    pub(crate) artifact: AutonomousLaneBlockArtifact,
    /// Current synthetic proposal after replaying the durable NewView proof suffix.
    pub(crate) current_proposal: LaneBlockProposalV1,
    /// Exact terminal identity which closed this attempt.
    pub(crate) retirement: AutonomousLaneSlotRetirementV1,
}
impl Kura {
    /// Read one exact, durably retired autonomous production attempt.
    ///
    /// Unlike [`Self::read_autonomous_lane_slot_retirement`], this lookup does
    /// not follow the mutable latest-attempt pointer. It addresses the immutable
    /// attempt by lane height and global proposal height, then fully revalidates
    /// its payload, NewView suffix, active geometry, network context, and exact
    /// [`AutonomousLaneSlotRetirementV1`]. The read never promotes or removes a
    /// recovery temporary.
    pub(crate) fn read_autonomous_lane_retired_attempt(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        proposal_height: u64,
        expected_network_id: iroha_data_model::NetworkId,
        expected_epoch: u64,
    ) -> Result<Option<AutonomousLaneRetiredAttempt>> {
        let _prune_guard = self.prune_lock.lock();
        self.ensure_prune_recovery_not_required()?;
        let _canonical_chain_guard = self.canonical_chain_lock.lock();
        let _geometry_guard = self.lane_geometry_lock.lock();
        let entry = self.lane_storage_entry(lane_id)?;
        let _sidecar_guard = self.sidecar_lock.lock();
        let Some(record) = self.read_autonomous_lane_block_attempt_record_locked(
            &entry,
            lane_id,
            lane_block_height,
            proposal_height,
            expected_network_id,
            expected_epoch,
            None,
        )?
        else {
            return Ok(None);
        };
        let AutonomousLaneBlockDurableRecord {
            artifact,
            retirement,
            view_state_path,
        } = record;
        let Some(retirement) = retirement else {
            return Ok(None);
        };
        if retirement != AutonomousLaneSlotRetirementV1::from_payload(&artifact.executable_payload)
        {
            return Err(Self::invalid_lane_artifact_error(
                view_state_path,
                "autonomous lane retirement does not match its exact durable attempt",
            ));
        }
        let current_proposal = Self::validate_autonomous_lane_block_artifact(
            &artifact,
            expected_network_id,
            expected_epoch,
        )
        .map_err(|message| Self::invalid_lane_artifact_error(view_state_path, message))?;
        Ok(Some(AutonomousLaneRetiredAttempt {
            artifact,
            current_proposal,
            retirement,
        }))
    }
}
