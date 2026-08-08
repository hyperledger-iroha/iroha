#[cfg(test)]
pub(crate) fn peer_key_policy_test_guard() -> TestLockGuard {
    reentrant_test_guard(&PEER_KEY_POLICY_TEST_LOCK)
}

#[cfg(test)]
pub(crate) fn local_removed_test_guard() -> TestLockGuard {
    reentrant_test_guard(&LOCAL_REMOVED_TEST_LOCK)
}

#[cfg(test)]
pub(crate) fn lane_relay_test_guard() -> std::sync::MutexGuard<'static, ()> {
    LANE_RELAY_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("lane relay test lock poisoned")
}

#[cfg(test)]
/// Reset settlement telemetry counters for isolated tests.
pub fn settlement_status_reset_for_tests() {
    *lock_operator_status_slot(settlement_status_slot(), "settlement status") =
        SettlementStatusState::default();
}

#[cfg(test)]
/// Reset process-local telemetry compatibility and lane-adapter diagnostics.
pub(crate) fn reset_rbc_backlog_stats_for_tests() {
    let _guard = rbc_status_test_guard();
    for counter in [
        &LAST_PROPOSE_MS,
        &LAST_COLLECT_DA_MS,
        &LAST_COLLECT_PREVOTE_MS,
        &LAST_COLLECT_PRECOMMIT_MS,
        &LAST_COLLECT_AGG_MS,
        &LAST_COMMIT_MS,
        &MAX_PROPOSE_MS,
        &MAX_COLLECT_DA_MS,
        &MAX_COLLECT_PREVOTE_MS,
        &MAX_COLLECT_PRECOMMIT_MS,
        &MAX_COLLECT_AGG_MS,
        &MAX_COMMIT_MS,
        &LAST_PROPOSE_EMA_MS,
        &LAST_COLLECT_DA_EMA_MS,
        &LAST_COLLECT_PREVOTE_EMA_MS,
        &LAST_COLLECT_PRECOMMIT_EMA_MS,
        &LAST_COLLECT_AGG_EMA_MS,
        &LAST_COMMIT_EMA_MS,
        &LAST_PIPELINE_TOTAL_EMA_MS,
        &GOSSIP_FALLBACK_TOTAL,
        &BLOCK_CREATED_DROPPED_BY_LOCK_TOTAL,
        &BLOCK_CREATED_HINT_MISMATCH_TOTAL,
        &BLOCK_CREATED_PROPOSAL_MISMATCH_TOTAL,
    ] {
        counter.store(0, Ordering::Relaxed);
    }
    *lock_operator_status_slot(availability_slot(), "availability vote stats") =
        AvailabilityStats::default();
    lock_operator_status_slot(qc_latency_slot(), "QC latency stats").clear();
    *lock_operator_status_slot(rbc_backlog_slot(), "RBC backlog snapshot") =
        RbcBacklogSnapshot::default();
    *lock_operator_status_slot(pending_rbc_slot(), "pending RBC snapshot") =
        PendingRbcSnapshot::default();
    lock_operator_status_slot(lane_activity_slot(), "lane activity snapshot").clear();
    lock_operator_status_slot(dataspace_activity_slot(), "dataspace activity snapshot").clear();
    *lock_operator_status_slot(pipeline_execution_slot(), "pipeline execution snapshot") =
        PipelineExecutionSnapshot::default();
    *lock_operator_status_slot(access_set_source_slot(), "access-set source snapshot") =
        AccessSetSourceSummary::default();
    PIPELINE_CONFLICT_RATE_BPS.store(0, Ordering::Relaxed);
}
