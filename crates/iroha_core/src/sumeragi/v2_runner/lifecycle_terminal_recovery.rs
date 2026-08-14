/// Immutable Queue boundary spanning Kura-only lane-evidence startup repair.
///
/// The runner's one-shot reconciliation flag is independent of Queue's
/// durable publication gate. An empty checked replay has no owners to
/// quarantine, so Queue correctly publishes `false`; a non-empty replay must
/// remain quarantined. Revalidation prevents evidence repair from racing or
/// masking Queue ownership/gate drift before reservation planning.
struct LaneApplicationEvidenceRepairQueueFence {
    snapshot: crate::queue::LaneQueueReservationReconciliationSnapshotV1,
    quarantine: bool,
}
impl LaneApplicationEvidenceRepairQueueFence {
    fn capture(queue: &Queue) -> Result<Self, V2RunnerError> {
        let snapshot = queue
            .lane_reservation_reconciliation_snapshot()
            .map_err(V2ReservationLifecycleError::from)?;
        let quarantine = queue.lane_reservation_startup_reconciliation_pending();
        if quarantine != !snapshot.is_empty() {
            return Err(V2RunnerError::Service(
                "lane application evidence repair reached startup with a Queue gate inconsistent with its exact replay snapshot"
                    .to_owned(),
            ));
        }
        Ok(Self {
            snapshot,
            quarantine,
        })
    }
    fn revalidate(&self, queue: &Queue) -> Result<(), V2RunnerError> {
        let snapshot = queue
            .lane_reservation_reconciliation_snapshot()
            .map_err(V2ReservationLifecycleError::from)?;
        if snapshot != self.snapshot
            || queue.lane_reservation_startup_reconciliation_pending() != self.quarantine
        {
            return Err(V2RunnerError::Service(
                "lane application evidence repair observed Queue ownership or publication-gate drift"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}
fn reconcile_lifecycle_terminal_outcomes_before_queue_planning(
    output_guard: &ConsensusOutputGuard,
    state: &State,
    queue: &Queue,
    kura: &Kura,
    context: &wire::HeightContext,
) -> Result<AutonomousLifecycleDeferredTerminalRecoveryHandoff, V2RunnerError> {
    // A crash may leave Kura's authenticated terminal source Pending after
    // Queue already forgot its final owner. Close only source units proven
    // Queue-empty before taking the planner receipt; whole carriers with any
    // surviving owner remain opaque deferred work for normal reconciliation.
    let recovery = output_guard
        .begin_fail_stop_operation()
        .ok_or(V2RunnerError::RestartRequired)?;
    let summary =
        reconcile_pending_autonomous_lifecycle_terminal_outcomes(state, queue, kura, context)
            .map_err(V2RunnerError::Service)?;
    recovery.complete();
    if summary.completed_outcomes() != 0 {
        iroha_logger::info!(
            completed_outcomes = summary.completed_outcomes(),
            finalized_reservations = summary.finalized_reservations(),
            "completed durable autonomous lifecycle terminal outcomes before Queue planning"
        );
    }
    if summary.deferred_pending_groups() != 0 {
        iroha_logger::info!(
            deferred_pending_groups = summary.deferred_pending_groups(),
            "deferred Queue-owned terminal outcomes into normal startup reconciliation"
        );
    }
    Ok(summary.into_deferred_terminal_recovery())
}
