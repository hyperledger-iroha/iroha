#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestLockOwner {
    Task(tokio::task::Id),
    Thread(std::thread::ThreadId),
}
#[cfg(test)]
thread_local! {
    static TEST_LOCK_OWNER_OVERRIDE: std::cell::Cell<Option<TestLockOwner>> =
        const { std::cell::Cell::new(None) };
}
#[cfg(test)]
impl TestLockOwner {
    fn current() -> Self {
        if let Some(owner) = TEST_LOCK_OWNER_OVERRIDE.with(std::cell::Cell::get) {
            return owner;
        }
        tokio::task::try_id().map_or_else(|| Self::Thread(std::thread::current().id()), Self::Task)
    }
}
#[cfg(test)]
#[derive(Default)]
struct TestLockState {
    owner: Option<TestLockOwner>,
    depth: usize,
}
#[cfg(test)]
#[derive(Default)]
struct TestLock {
    state: Mutex<TestLockState>,
    cvar: Condvar,
}
#[cfg(test)]
pub(crate) struct TestLockGuard {
    lock: &'static TestLock,
    owner: TestLockOwner,
}
#[cfg(test)]
impl Drop for TestLockGuard {
    fn drop(&mut self) {
        let mut state = self
            .lock
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.owner == Some(self.owner) {
            state.depth = state.depth.saturating_sub(1);
            if state.depth == 0 {
                state.owner = None;
                self.lock.cvar.notify_one();
            }
        }
    }
}
#[cfg(test)]
static STATUS_TEST_GLOBAL_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static RBC_STATUS_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static COMMIT_HISTORY_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static MODE_TAGS_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static PEER_KEY_POLICY_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static LOCAL_REMOVED_TEST_LOCK: OnceLock<TestLock> = OnceLock::new();
#[cfg(test)]
static LANE_RELAY_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
#[cfg(test)]
fn canonical_test_lock(_: &'static OnceLock<TestLock>) -> &'static TestLock {
    STATUS_TEST_GLOBAL_LOCK.get_or_init(TestLock::default)
}
#[cfg(test)]
fn reentrant_test_guard(lock: &'static OnceLock<TestLock>) -> TestLockGuard {
    let owner = TestLockOwner::current();
    let lock = canonical_test_lock(lock);
    let mut state = lock
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    loop {
        match state.owner {
            None => {
                state.owner = Some(owner);
                state.depth = 1;
                break;
            }
            Some(current) if current == owner => {
                state.depth = state.depth.saturating_add(1);
                break;
            }
            Some(_) => {
                state = lock
                    .cvar
                    .wait(state)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        }
    }
    TestLockGuard { lock, owner }
}
#[cfg(test)]
fn try_reentrant_test_guard(lock: &'static OnceLock<TestLock>) -> Option<TestLockGuard> {
    let owner = TestLockOwner::current();
    let lock = canonical_test_lock(lock);
    let mut state = lock
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match state.owner {
        None => {
            state.owner = Some(owner);
            state.depth = 1;
            Some(TestLockGuard { lock, owner })
        }
        Some(current) if current == owner => {
            state.depth = state.depth.saturating_add(1);
            Some(TestLockGuard { lock, owner })
        }
        Some(_) => None,
    }
}
#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct NexusFeeTestLock;
#[cfg(test)]
pub(crate) struct NexusFeeTestGuard {
    _guard: TestLockGuard,
}
#[cfg(test)]
impl NexusFeeTestLock {
    pub(crate) fn lock(&'static self) -> Result<NexusFeeTestGuard, std::convert::Infallible> {
        Ok(NexusFeeTestGuard {
            _guard: reentrant_test_guard(&RBC_STATUS_TEST_LOCK),
        })
    }
}
#[cfg(test)]
/// Serialize every process-wide v2 status mutation with tests that need a
/// stable clear/publish/observe window.
///
/// This is a synchronous, owner-reentrant test lease. Do not move it to another
/// task or thread for nested use, hold it across `.await`, or wait for a child
/// which can call a guarded status mutation; each of those patterns can prevent
/// the original owner from releasing the lease.
pub(crate) fn rbc_status_test_guard() -> TestLockGuard {
    reentrant_test_guard(&RBC_STATUS_TEST_LOCK)
}
#[cfg(test)]
/// Serialize tests that mutate archival commit history.
pub(crate) fn commit_history_test_guard() -> TestLockGuard {
    reentrant_test_guard(&COMMIT_HISTORY_TEST_LOCK)
}
#[cfg(test)]
/// Serialize tests that mutate archival mode tags.
pub(crate) fn mode_tags_test_guard() -> TestLockGuard {
    reentrant_test_guard(&MODE_TAGS_TEST_LOCK)
}
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
