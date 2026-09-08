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
    /// authenticated canonical ownership snapshots.
    pub(crate) fn certified_lane_block_session_predecessor_is_applied(
        &self,
        session: &crate::lane_consensus::CommittedLaneBlockSession,
    ) -> Result<bool, MergeLedgerCommitError> {
        if session.prepare_qc.payload_availability_qc.is_some() {
            self.certified_autonomous_lane_block_predecessor_is_globally_applied(&session.proposal)
        } else {
            self.certified_lane_block_predecessor_is_applied_or_snapshot_anchored(&session.proposal)
        }
    }
    /// Native application closes its shared prefix to current-height lane
    /// voting. Historical certificate recovery may still need an ordinary gap
    /// inside that prefix, so only exact authenticated Native coordinates close
    /// a historical slot. Applied-session observation remains independent.
    pub(crate) fn native_amx_participant_application_closes_lane_slot(
        &self,
        proposal: &iroha_data_model::block::consensus::LaneBlockProposalV1,
        active_proposal_height: u64,
    ) -> Result<bool, MergeLedgerCommitError> {
        let descriptor = &proposal.descriptor;
        let snapshot = self.native_amx_participant_application_snapshot()?;
        if descriptor.proposal_height < active_proposal_height {
            return Ok(snapshot.applied_slots.contains(&(
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.lane_block_height,
            )));
        }
        Ok(snapshot.applied.iter().any(|marker| {
            marker.lane_id == descriptor.lane_id
                && marker.dataspace_id == descriptor.dataspace_id
                && marker.lane_incarnation == descriptor.lane_incarnation
                && marker.lane_block_height >= descriptor.lane_block_height
        }))
    }
    /// Classify exact autonomous application or its predecessor at one published State frontier.
    ///
    /// The two predicates overlap across publication: before a merge the predecessor is applied;
    /// afterwards the proposal itself is applied. Reading them through separate raw world views
    /// can observe neither while a State publication advances between the reads. Hold the same
    /// fence as publication through both checks and their authenticated receipt reads. This does
    /// not turn missing, conflicting or malformed application evidence into authority.
    pub(crate) fn certified_autonomous_lane_block_or_predecessor_is_globally_applied(
        &self,
        proposal: &iroha_data_model::block::consensus::LaneBlockProposalV1,
    ) -> Result<bool, MergeLedgerCommitError> {
        let _state_commit = self.state_commit_lock.lock();
        Ok(
            self.certified_autonomous_lane_block_is_globally_applied(proposal)?
                || self
                    .certified_autonomous_lane_block_predecessor_is_globally_applied(proposal)?,
        )
    }
    /// Require canonical economic application of an autonomous proposal.
    ///
    /// A lane ownership artifact proves payload routing, not WSV application.
    /// Autonomous application is therefore admitted only by an exact
    /// merge-log/carrier receipt or the replicated per-incarnation WSV frontier.
    /// Malformed replicated marker bytes fail closed even if local Kura happens
    /// to contain a receipt.
    pub(crate) fn certified_autonomous_lane_block_is_globally_applied(
        &self,
        proposal: &iroha_data_model::block::consensus::LaneBlockProposalV1,
    ) -> Result<bool, MergeLedgerCommitError> {
        let descriptor = &proposal.descriptor;
        if descriptor.lane_block_height == 0 {
            return Ok(false);
        }
        let world = self.world.view();
        let frontier = Self::canonical_merged_lane_frontier_from_world(
            &world,
            descriptor.lane_id,
            descriptor.dataspace_id,
            descriptor.lane_incarnation,
        )?;
        if frontier
            == (
                descriptor.lane_block_height,
                Some(descriptor.descriptor_hash),
            )
        {
            return Ok(true);
        }
        let receipt = self
            .kura
            .read_lane_application_receipt(descriptor.lane_id, descriptor.lane_block_height)
            .map_err(|error| {
                MergeLedgerCommitError::ExecutionMarkerConflict(format!(
                    "failed to authenticate autonomous lane application receipt: {error}"
                ))
            })?;
        Ok(receipt.is_some_and(|receipt| {
            receipt.proposal == *proposal
                && receipt.format
                    == crate::kura::LaneBlockApplicationReceiptArtifactFormat::MergeExecution
        }))
    }
    /// Require canonical economic application of the exact predecessor of an
    /// autonomous proposal.
    ///
    /// The predecessor may itself have reached WSV through an ordinary
    /// canonical block (`Current` receipt) or through an autonomous merge
    /// carrier.
    ///
    /// Unlike the ordinary helper, this deliberately ignores hash-only lane
    /// ownership snapshots. Those snapshots authenticate a canonical carrier
    /// identity but cannot prove that its autonomous effects crossed the WSV
    /// application boundary.
    pub(crate) fn certified_autonomous_lane_block_predecessor_is_globally_applied(
        &self,
        proposal: &iroha_data_model::block::consensus::LaneBlockProposalV1,
    ) -> Result<bool, MergeLedgerCommitError> {
        let descriptor = &proposal.descriptor;
        let previous_height = descriptor.previous_lane_block_height;
        if previous_height == 0 {
            if descriptor.lane_block_height != 1
                || descriptor.previous_lane_block_descriptor_hash.is_some()
            {
                return Ok(false);
            }
        } else if descriptor.previous_lane_block_descriptor_hash.is_none()
            || previous_height.checked_add(1) != Some(descriptor.lane_block_height)
        {
            return Ok(false);
        }
        // A replicated or hash-only frontier cannot bypass an interrupted or
        // corrupt Native publication in the same shared lane sequence.
        let native_snapshot = self.native_amx_participant_application_snapshot()?;
        if native_snapshot
            .blocked
            .contains_key(&(descriptor.lane_id, descriptor.dataspace_id))
        {
            return Ok(false);
        }
        // An empty predecessor is still an application-authority decision:
        // a pending or corrupt Native first slot must not become a fresh lane.
        let Some(previous_descriptor_hash) = descriptor.previous_lane_block_descriptor_hash else {
            return Ok(true);
        };
        let world = self.world.view();
        let frontier = Self::canonical_merged_lane_frontier_from_world(
            &world,
            descriptor.lane_id,
            descriptor.dataspace_id,
            descriptor.lane_incarnation,
        )?;
        if frontier.0 >= previous_height {
            return Ok(frontier == (previous_height, Some(previous_descriptor_hash)));
        }
        self.lane_block_predecessor_has_authenticated_receipt(proposal, &native_snapshot)
    }
    /// Authenticate a predecessor receipt independently of the untrusted candidate.
    /// Ordinary receipts and Native participant histories share one lane sequence.
    /// Authenticate all Native publication state before choosing either source,
    /// so pending or corrupt Native evidence cannot become an ordinary fallback.
    fn lane_block_predecessor_has_authenticated_receipt(
        &self,
        proposal: &iroha_data_model::block::consensus::LaneBlockProposalV1,
        native_snapshot: &NativeAmxParticipantApplicationSnapshot,
    ) -> Result<bool, MergeLedgerCommitError> {
        let descriptor = &proposal.descriptor;
        let previous_height = descriptor.previous_lane_block_height;
        let Some(previous_descriptor_hash) = descriptor.previous_lane_block_descriptor_hash else {
            return Ok(false);
        };
        if previous_height == 0
            || previous_height.checked_add(1) != Some(descriptor.lane_block_height)
        {
            return Ok(false);
        }
        let receipt = self
            .kura
            .read_lane_application_receipt(descriptor.lane_id, previous_height)
            .map_err(|error| {
                MergeLedgerCommitError::ExecutionMarkerConflict(format!(
                    "failed to authenticate lane predecessor receipt: {error}"
                ))
            })?;
        let native_history = self
            .kura
            .read_native_amx_participant_application_history(descriptor.lane_id)
            .map_err(MergeLedgerCommitError::Persistence)?;
        let native_frontier = native_snapshot.applied.iter().find(|marker| {
            marker.lane_id == descriptor.lane_id
                && marker.dataspace_id == descriptor.dataspace_id
                && marker.lane_incarnation == descriptor.lane_incarnation
                && marker.lane_block_height >= previous_height
        });
        let native_receipt =
            native_frontier.and_then(|_| match native_history.get(previous_height) {
                Some(crate::kura::NativeAmxParticipantApplicationObservation::Applied(receipt)) => {
                    Some(receipt)
                }
                _ => None,
            });
        if let (Some(ordinary), Some(native)) = (receipt.as_ref(), native_receipt) {
            if ordinary.proposal != native.participant_proposal {
                return Err(MergeLedgerCommitError::ExecutionMarkerConflict(
                    "ordinary and Native receipts disagree at the same shared lane height"
                        .to_owned(),
                ));
            }
        }
        let matches_predecessor =
            |previous: &iroha_data_model::block::consensus::LaneBlockProposalV1| {
                let previous = &previous.descriptor;
                previous.lane_id == descriptor.lane_id
                    && previous.dataspace_id == descriptor.dataspace_id
                    && previous.lane_incarnation == descriptor.lane_incarnation
                    && previous.lane_block_height == previous_height
                    && previous.descriptor_hash == previous_descriptor_hash
                    && previous.proposal_height < descriptor.proposal_height
            };
        Ok(receipt
            .as_ref()
            .is_some_and(|receipt| matches_predecessor(&receipt.proposal))
            || native_receipt.is_some_and(|receipt| {
                matches_predecessor(&receipt.participant_proposal)
                    && receipt.application_block_height < descriptor.proposal_height
            }))
    }
}
