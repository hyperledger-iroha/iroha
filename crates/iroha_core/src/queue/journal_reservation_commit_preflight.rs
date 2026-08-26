impl QueuePlanJournal {
    /// Observe the exact post-replay V1 image and classify every V1 startup owner.
    ///
    /// This receipt is evidence only. It authenticates one direct journal and
    /// parent identity, byte length, content digest, complete live-record root,
    /// and exact per-owner V1 phase.
    pub(super) fn observe_startup_replay_receipt(
        &self,
        phases: &[LaneQueueReservationRecoveryPhaseV1],
    ) -> io::Result<QueuePlanStartupReplayReceiptV1> {
        self.observe_startup_replay_receipt_with_finalized_absence(phases, &[])
    }
    /// Validate active V1 phases and already-finalized carrier keys against one
    /// immutable V1 journal snapshot.
    ///
    /// Active phases remain bounded by the journal's maximum live records.
    /// Finalized keys may include authenticated absent siblings from multiple
    /// carriers and are bounded by Queue's outer carrier preflight instead. The
    /// single replay plus before/after content verification prevents a later
    /// carrier conflict from following an earlier mutation or a different file
    /// image.
    pub(super) fn observe_startup_replay_receipt_with_finalized_absence(
        &self,
        phases: &[LaneQueueReservationRecoveryPhaseV1],
        finalized_keys: &[LaneQueueReservationKeyV1],
    ) -> io::Result<QueuePlanStartupReplayReceiptV1> {
        self.ensure_healthy()?;
        #[cfg(test)]
        if self.take_fault(QueuePlanJournalTestFault::StartupReplayReceiptObserve) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "injected queue-plan startup replay receipt observation failure",
            ));
        }
        if phases.len() > self.limits.max_live_records {
            return Err(invalid_data(
                "queue-plan startup phase coverage exceeds the configured owner bound",
            ));
        }
        let mut owner_hashes = BTreeSet::new();
        let mut entrypoints = BTreeSet::new();
        for phase in phases {
            phase.key.validate().map_err(invalid_data)?;
            if !owner_hashes.insert(phase.key.entrypoint_hash) {
                return Err(invalid_data(
                    "queue-plan startup phase coverage contains a duplicate reservation owner",
                ));
            }
            if !entrypoints.insert(phase.key.entrypoint_hash.clone()) {
                return Err(invalid_data(
                    "queue-plan startup phase coverage contains a duplicate entrypoint",
                ));
            }
            match phase.reservation_phase {
                LaneQueueReservationOwnerPhaseV1::CommitBarrier => {}
                LaneQueueReservationOwnerPhaseV1::Live
                | LaneQueueReservationOwnerPhaseV1::ReleasePrepared
                | LaneQueueReservationOwnerPhaseV1::ReleaseCompleted => {
                    if phase.queue_plan_phase != QueuePlanReservationPhaseV1::Live
                        || phase.plan_tombstone_marked
                    {
                        return Err(invalid_data(
                            "non-commit reservation owner must retain one live unmarked QueuePlan claim",
                        ));
                    }
                }
            }
            if phase.plan_tombstone_marked
                && phase.queue_plan_phase != QueuePlanReservationPhaseV1::Tombstoned
            {
                return Err(invalid_data(
                    "V1 PlanTombstoned marker conflicts with a claimed live V1 phase",
                ));
            }
        }
        for key in finalized_keys {
            key.validate().map_err(invalid_data)?;
            if !owner_hashes.insert(key.entrypoint_hash) {
                return Err(invalid_data(
                    "finalized reservation preflight contains a duplicate owner",
                ));
            }
            if !entrypoints.insert(key.entrypoint_hash.clone()) {
                return Err(invalid_data(
                    "finalized reservation preflight contains a duplicate entrypoint",
                ));
            }
        }
        let mut replay = self.prepare_replay_with_removed_entrypoints(Some(&entrypoints))?;
        replay.verify_snapshot_content()?;
        for phase in phases {
            let actual = if let Some(live) = replay.live_positions.get(&phase.key.entrypoint_hash) {
                live.validate_global_admission_for_reservation_commit(&phase.key)?;
                if phase.plan_tombstone_marked {
                    return Err(invalid_data(
                        "durable V1 PlanTombstoned marker conflicts with a live V1 claim",
                    ));
                }
                QueuePlanReservationPhaseV1::Live
            } else if let Some(removed) = replay.removed_positions.get(&phase.key.entrypoint_hash) {
                removed.validate_global_admission_for_reservation_commit(&phase.key)?;
                QueuePlanReservationPhaseV1::Tombstoned
            } else if phase.plan_tombstone_marked {
                QueuePlanReservationPhaseV1::Tombstoned
            } else {
                return Err(invalid_data(
                    "unmarked reservation owner is neither live nor exactly tombstoned in V1",
                ));
            };
            if actual != phase.queue_plan_phase {
                return Err(invalid_data(
                    "queue-plan startup phase disagrees with the exact V1 journal image",
                ));
            }
        }
        for key in finalized_keys {
            if let Some(live) = replay.live_positions.get(&key.entrypoint_hash) {
                live.validate_global_admission_for_reservation_commit(key)?;
                return Err(invalid_data(
                    "finalized reservation retains a live V1 QueuePlan owner",
                ));
            }
            if let Some(removed) = replay.removed_positions.get(&key.entrypoint_hash) {
                removed.validate_global_admission_for_reservation_commit(key)?;
            }
        }
        let live_claims =
            replay
                .live_positions
                .values()
                .map(|live| QueuePlanStartupLiveClaimIdentityV1 {
                    entrypoint_hash: live.record.entrypoint_hash.clone(),
                    routing_plan_digest: live.plan_digest,
                    journal_record_digest: live.claim_digest,
                });
        let (live_record_count, live_record_root) =
            queue_plan_startup_live_record_root(live_claims)?;
        let (reservation_phase_count, reservation_phase_root) =
            queue_plan_startup_reservation_phase_root(phases)?;
        replay.verify_snapshot_content()?;
        Ok(QueuePlanStartupReplayReceiptV1 {
            file_identity: replay.file_identity,
            parent_identity: replay.parent_identity,
            snapshot_len: replay.snapshot_len,
            snapshot_digest: replay.snapshot_digest,
            live_record_count,
            live_record_root,
            reservation_phase_count,
            reservation_phase_root,
        })
    }
}
