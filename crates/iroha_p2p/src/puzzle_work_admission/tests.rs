//! Deterministic scheduling regressions for bounded outbound puzzle searches.

use std::{
    sync::{atomic::AtomicUsize, mpsc as std_mpsc},
    time::Duration,
};

use futures::FutureExt;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::timeout,
};

use super::*;

const TEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Default)]
struct WorkCounts {
    active: AtomicUsize,
    peak: AtomicUsize,
    created: AtomicUsize,
}

struct ActiveWork(Arc<WorkCounts>);

impl ActiveWork {
    fn new(counts: Arc<WorkCounts>) -> Self {
        let active = counts.active.fetch_add(1, Ordering::AcqRel) + 1;
        counts.peak.fetch_max(active, Ordering::AcqRel);
        Self(counts)
    }
}

impl Drop for ActiveWork {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::AcqRel);
    }
}

enum Command {
    Checkpoint(oneshot::Sender<(bool, bool)>),
    Complete(usize),
    Fail,
    Panic,
}

struct Worker {
    index: usize,
    speculative: bool,
    commands: std_mpsc::Sender<Command>,
}

impl Worker {
    async fn checkpoint(&self) -> (bool, bool) {
        let (reply, result) = oneshot::channel();
        self.commands
            .send(Command::Checkpoint(reply))
            .expect("worker is evaluating");
        timeout(TEST_TIMEOUT, result)
            .await
            .expect("checkpoint must respond")
            .expect("checkpoint response")
    }

    fn complete(&self, value: usize) {
        self.commands
            .send(Command::Complete(value))
            .expect("worker is evaluating");
    }
}

struct Search {
    task: JoinHandle<Result<usize, Error>>,
    started: mpsc::UnboundedReceiver<Worker>,
}

impl Search {
    async fn next_worker(&mut self) -> Worker {
        timeout(TEST_TIMEOUT, self.started.recv())
            .await
            .expect("worker must be admitted")
            .expect("search must retain a worker")
    }

    async fn finish(self) -> Result<usize, Error> {
        timeout(TEST_TIMEOUT, self.task)
            .await
            .expect("search must finish")
            .expect("coordinator must not panic")
    }
}

fn admission(capacity: usize) -> Arc<SoranetPuzzleWorkAdmission> {
    Arc::new(SoranetPuzzleWorkAdmission::new(
        NonZeroUsize::new(capacity).expect("nonzero test capacity"),
        NonZeroUsize::new(1).expect("nonzero verification capacity"),
    ))
}

fn start_search(admission: Arc<SoranetPuzzleWorkAdmission>, counts: Arc<WorkCounts>) -> Search {
    let (started, receiver) = mpsc::unbounded_channel();
    let mut index = 0;
    let task = tokio::spawn(run_soranet_outbound_search(admission, move || {
        let worker_index = index;
        index += 1;
        counts.created.fetch_add(1, Ordering::AcqRel);
        let started = started.clone();
        let counts = Arc::clone(&counts);
        move |control: SoranetOutboundWorkControl| {
            let _active = ActiveWork::new(counts);
            let (commands, receiver) = std_mpsc::channel();
            started
                .send(Worker {
                    index: worker_index,
                    speculative: control.speculative,
                    commands,
                })
                .expect("test retains the worker receiver");
            loop {
                let command = receiver
                    .recv_timeout(TEST_TIMEOUT)
                    .expect("test releases every current evaluation");
                let value = match command {
                    Command::Checkpoint(reply) => {
                        let keep_running = control.should_continue();
                        let _ = reply.send((keep_running, control.yielded()));
                        if keep_running {
                            continue;
                        }
                        None
                    }
                    Command::Complete(value) => control.should_continue().then_some(value),
                    Command::Fail => {
                        return Err(Error::HandshakeSoranet(
                            "injected search failure".to_owned(),
                        ));
                    }
                    Command::Panic => panic!("injected blocking search panic"),
                };
                return match value {
                    Some(value) => Ok(SoranetOutboundWorkOutcome::Completed(value)),
                    None if control.yielded() => Ok(SoranetOutboundWorkOutcome::Yielded),
                    None => Err(Error::HandshakeSoranet("cancelled test search".to_owned())),
                };
            }
        }
    }));
    Search {
        task,
        started: receiver,
    }
}

async fn wait_for_waiters(admission: &SoranetPuzzleWorkAdmission, expected: usize) {
    timeout(TEST_TIMEOUT, async {
        while admission.outbound_primary_waiters.load(Ordering::Acquire) != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("primary waiter accounting must converge");
}

async fn wait_for_idle(admission: &SoranetPuzzleWorkAdmission) {
    let permits = u32::try_from(admission.outbound_mint_capacity.get()).expect("test capacity");
    let all = timeout(TEST_TIMEOUT, admission.outbound_mint.acquire_many(permits))
        .await
        .expect("all current evaluations must exit")
        .expect("open gate");
    drop(all);
}

#[tokio::test]
async fn outbound_search_capacity_one_serializes_primaries_and_removes_cancelled_waiters() {
    let admission = admission(1);
    let counts = Arc::new(WorkCounts::default());
    let mut first = start_search(Arc::clone(&admission), Arc::clone(&counts));
    let first_worker = first.next_worker().await;
    assert!(!first_worker.speculative);
    assert_eq!(first_worker.index, 0);
    let cancelled = start_search(Arc::clone(&admission), Arc::clone(&counts));
    wait_for_waiters(&admission, 1).await;
    cancelled.task.abort();
    assert!(
        cancelled
            .task
            .await
            .expect_err("cancelled owner")
            .is_cancelled()
    );
    wait_for_waiters(&admission, 0).await;

    let mut next = start_search(Arc::clone(&admission), Arc::clone(&counts));
    wait_for_waiters(&admission, 1).await;
    assert_eq!(counts.created.load(Ordering::Acquire), 1);
    first_worker.complete(11);
    assert_eq!(first.finish().await.expect("first credential"), 11);
    let next_worker = next.next_worker().await;
    assert!(!next_worker.speculative);
    next_worker.complete(22);
    assert_eq!(next.finish().await.expect("next credential"), 22);
    wait_for_idle(&admission).await;
    assert_eq!(counts.created.load(Ordering::Acquire), 2);
    assert_eq!(counts.peak.load(Ordering::Acquire), 1);
    assert_eq!(
        admission.outbound_primary_waiters.load(Ordering::Acquire),
        0
    );
}

#[tokio::test]
async fn outbound_search_helpers_yield_without_aborting_primary_and_refill_after_contention() {
    let admission = admission(2);
    let counts = Arc::new(WorkCounts::default());
    let mut first = start_search(Arc::clone(&admission), Arc::clone(&counts));
    let mut initial = [first.next_worker().await, first.next_worker().await];
    initial.sort_by_key(|worker| worker.speculative);
    let [primary, helper] = initial;
    assert!(!primary.speculative);
    assert!(helper.speculative);
    let mut second = start_search(Arc::clone(&admission), Arc::clone(&counts));
    wait_for_waiters(&admission, 1).await;
    assert_eq!(primary.checkpoint().await, (true, false));
    assert_eq!(helper.checkpoint().await, (false, true));
    let second_primary = second.next_worker().await;
    assert!(!second_primary.speculative);
    assert!(
        !first.task.is_finished(),
        "yielding a helper is not a failure"
    );
    assert_eq!(admission.outbound_mint.available_permits(), 0);

    second_primary.complete(22);
    assert_eq!(second.finish().await.expect("second credential"), 22);
    let refill = first.next_worker().await;
    assert!(refill.speculative);
    assert!(refill.index > helper.index, "refill creates a fresh search");
    refill.complete(33);
    assert_eq!(first.finish().await.expect("refilled credential"), 33);
    assert_eq!(primary.checkpoint().await, (false, false));
    wait_for_idle(&admission).await;
    assert_eq!(counts.peak.load(Ordering::Acquire), 2);
}

#[tokio::test]
async fn outbound_search_winner_cancels_losers_without_releasing_memory_or_registry_owner_early() {
    let admission = admission(3);
    let gate = Arc::clone(&admission.outbound_mint);
    let owner = Arc::downgrade(&admission);
    let counts = Arc::new(WorkCounts::default());
    let mut search = start_search(Arc::clone(&admission), Arc::clone(&counts));
    let first = search.next_worker().await;
    let second = search.next_worker().await;
    let third = search.next_worker().await;
    assert_eq!(gate.available_permits(), 0);
    assert_eq!(counts.created.load(Ordering::Acquire), 3);
    second.complete(42);
    assert_eq!(search.finish().await.expect("winning credential"), 42);
    assert_eq!(gate.available_permits(), 1, "losers are still evaluating");
    drop(admission);
    assert!(
        owner.upgrade().is_some(),
        "blocking losers retain the registry owner"
    );
    assert_eq!(first.checkpoint().await, (false, false));
    assert_eq!(third.checkpoint().await, (false, false));
    let all = timeout(TEST_TIMEOUT, gate.acquire_many(3))
        .await
        .expect("losers must eventually return their permits")
        .expect("open gate");
    drop(all);
    timeout(TEST_TIMEOUT, async {
        while owner.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("registry owner must be released after all work exits");
    assert_eq!(counts.peak.load(Ordering::Acquire), 3);
    assert_eq!(counts.active.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn outbound_search_bounds_pending_results_and_drops_unselected_completed_values() {
    let admission = admission(3);
    let counts = Arc::new(WorkCounts::default());
    let (started, mut workers) = mpsc::unbounded_channel();
    let work_counts = Arc::clone(&counts);
    let mut search = Box::pin(run_soranet_outbound_search(
        Arc::clone(&admission),
        move || {
            work_counts.created.fetch_add(1, Ordering::AcqRel);
            // This owner spans queued work, execution and its completed value.
            let owner = ActiveWork::new(Arc::clone(&work_counts));
            let started = started.clone();
            move |_control| {
                let (release, evaluation) = std_mpsc::channel();
                started.send(release).expect("test owns receiver");
                evaluation
                    .recv_timeout(TEST_TIMEOUT)
                    .expect("release evaluation");
                Ok(SoranetOutboundWorkOutcome::Completed(owner))
            }
        },
    ));
    assert!(search.as_mut().now_or_never().is_none());
    for _ in 0..3 {
        timeout(TEST_TIMEOUT, workers.recv())
            .await
            .expect("worker starts")
            .expect("release handle")
            .send(())
            .expect("release current evaluation");
    }
    // Keep the coordinator unpolled until all three evaluations have returned.
    wait_for_idle(&admission).await;
    assert_eq!(counts.active.load(Ordering::Acquire), 3);
    let winner = timeout(TEST_TIMEOUT, search)
        .await
        .expect("completed search")
        .expect("winning value");
    assert_eq!(counts.created.load(Ordering::Acquire), 3);
    assert_eq!(counts.peak.load(Ordering::Acquire), 3);
    timeout(TEST_TIMEOUT, async {
        while counts.active.load(Ordering::Acquire) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("detached completed losers must drop their values");
    assert_eq!(counts.active.load(Ordering::Acquire), 1);
    drop(winner);
    assert_eq!(counts.active.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn outbound_search_cancelled_waiter_does_not_keep_helpers_yielding() {
    let admission = admission(2);
    let counts = Arc::new(WorkCounts::default());
    let mut search = start_search(Arc::clone(&admission), Arc::clone(&counts));
    let mut workers = [search.next_worker().await, search.next_worker().await];
    workers.sort_by_key(|worker| worker.speculative);
    let [primary, helper] = workers;
    let waiting = start_search(Arc::clone(&admission), counts);
    wait_for_waiters(&admission, 1).await;
    waiting.task.abort();
    assert!(
        waiting
            .task
            .await
            .expect_err("cancelled waiter")
            .is_cancelled()
    );
    wait_for_waiters(&admission, 0).await;
    assert_eq!(helper.checkpoint().await, (true, false));
    helper.complete(42);
    assert_eq!(search.finish().await.expect("helper credential"), 42);
    assert_eq!(primary.checkpoint().await, (false, false));
    wait_for_idle(&admission).await;
}

#[tokio::test]
async fn outbound_search_owner_cancellation_keeps_all_evaluations_charged() {
    let admission = admission(3);
    let counts = Arc::new(WorkCounts::default());
    let mut search = start_search(Arc::clone(&admission), counts);
    let workers = [
        search.next_worker().await,
        search.next_worker().await,
        search.next_worker().await,
    ];
    search.task.abort();
    assert!(
        search
            .task
            .await
            .expect_err("cancelled owner")
            .is_cancelled()
    );
    assert_eq!(admission.outbound_mint.available_permits(), 0);
    for worker in workers {
        assert_eq!(worker.checkpoint().await, (false, false));
    }
    wait_for_idle(&admission).await;
}

#[tokio::test]
async fn outbound_search_failures_and_panics_fail_closed_and_cancel_siblings() {
    for command in [Command::Fail, Command::Panic] {
        let admission = admission(2);
        let counts = Arc::new(WorkCounts::default());
        let mut search = start_search(Arc::clone(&admission), counts);
        let failed = search.next_worker().await;
        let sibling = search.next_worker().await;
        failed.commands.send(command).expect("running worker");
        assert!(search.finish().await.is_err());
        assert_eq!(admission.outbound_mint.available_permits(), 1);
        assert_eq!(sibling.checkpoint().await, (false, false));
        wait_for_idle(&admission).await;
    }
}

#[tokio::test]
async fn outbound_search_closed_gate_fails_before_creating_work() {
    let admission = admission(3);
    admission.outbound_mint.close();
    let counts = Arc::new(WorkCounts::default());
    assert!(
        start_search(Arc::clone(&admission), Arc::clone(&counts))
            .finish()
            .await
            .is_err()
    );
    assert_eq!(counts.created.load(Ordering::Acquire), 0);
    assert_eq!(
        admission.outbound_primary_waiters.load(Ordering::Acquire),
        0
    );
}

#[test]
fn outbound_search_yield_reason_stays_latched_after_waiter_cancellation() {
    let admission = admission(2);
    let waiter = OutboundPrimaryWaiter::new(Arc::clone(&admission));
    let control = SoranetOutboundWorkControl {
        cancellation: SoranetAdmissionCancellation(Arc::new(AtomicBool::new(false))),
        admission,
        speculative: true,
        yielded: Cell::new(false),
    };
    assert!(!control.should_continue());
    drop(waiter);
    assert!(control.yielded());
    assert!(
        !control.should_continue(),
        "yielded searches cannot resume with stale ownership"
    );
}
