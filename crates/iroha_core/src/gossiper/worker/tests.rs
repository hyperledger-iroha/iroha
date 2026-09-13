//! Executor progress, exact ownership, and natural shutdown of the production gossip loop.
use super::*;
use crate::gossiper::GossipPlane;
use iroha_futures::supervisor::{Error as SupervisorError, Supervisor};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc as blocking_mpsc,
};
use tokio::{sync::oneshot, time::timeout};

// Existing physical-executor controls exercise the same mandatory worker;
// they do not create a deferred batch. Their publication dependency cannot wake.
fn start<F>(
    period: Duration,
    messages: mpsc::Receiver<RetainedGossip<Arc<TransactionGossip>>>,
    shutdown: ShutdownSignal,
    mut handle: F,
) -> Result<Child, TransactionGossiperStartError>
where
    F: FnMut(Work) + Send + 'static,
{
    super::start(
        period,
        messages,
        shutdown,
        |_| std::future::pending::<()>(),
        move |work| {
            handle(work);
            None
        },
    )
}

fn message() -> RetainedGossip<Arc<TransactionGossip>> {
    RetainedGossip::synthetic_for_test(Arc::new(TransactionGossip {
        txs: Vec::new(),
        routes: Vec::new(),
        plans: Vec::new(),
        plane: GossipPlane::Public,
    }))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn blocked_incoming_work_allows_executor_progress() {
    let mut supervisor = Supervisor::new();
    let shutdown = supervisor.shutdown_signal();
    let (sender, receiver) = mpsc::channel(1);
    let (entered_tx, entered_rx) = oneshot::channel();
    let (release_tx, release_rx) = blocking_mpsc::channel();
    let mut entered_tx = Some(entered_tx);
    let stop = shutdown.clone();
    supervisor.monitor(
        start(Duration::from_secs(60), receiver, shutdown, move |work| {
            if let Work::Incoming(_message) = work {
                entered_tx
                    .take()
                    .expect("one incoming work item")
                    .send(())
                    .unwrap();
                release_rx
                    .recv_timeout(Duration::from_secs(2))
                    .expect("another Tokio task must run while gossip work blocks");
                stop.send();
            }
        })
        .expect("valid multithreaded gossip worker"),
    );
    sender.send(message()).await.unwrap();
    // This task must run on the sole executor worker, not the test's block_on thread.
    let release = tokio::spawn(async move {
        entered_rx.await.unwrap();
        tokio::task::yield_now().await;
        release_tx.send(()).unwrap();
    });
    timeout(Duration::from_secs(5), supervisor.start())
        .await
        .unwrap()
        .unwrap();
    release.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn blocked_periodic_work_allows_executor_progress() {
    let mut supervisor = Supervisor::new();
    let shutdown = supervisor.shutdown_signal();
    let (_sender, receiver) = mpsc::channel(1);
    let (entered_tx, entered_rx) = oneshot::channel();
    let (release_tx, release_rx) = blocking_mpsc::channel();
    let mut entered_tx = Some(entered_tx);
    let stop = shutdown.clone();
    supervisor.monitor(
        start(Duration::from_secs(60), receiver, shutdown, move |work| {
            assert!(matches!(work, Work::Tick));
            entered_tx
                .take()
                .expect("one periodic work item")
                .send(())
                .unwrap();
            release_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("executor must make progress");
            stop.send();
        })
        .expect("valid multithreaded gossip worker"),
    );
    let release = tokio::spawn(async move {
        entered_rx.await.unwrap();
        tokio::task::yield_now().await;
        release_tx.send(()).unwrap();
    });
    timeout(Duration::from_secs(5), supervisor.start())
        .await
        .unwrap()
        .unwrap();
    release.await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cooperative_shutdown_waits_for_owned_message_and_skips_pending_work() {
    let mut supervisor = Supervisor::new();
    let shutdown = supervisor.shutdown_signal();
    let (sender, receiver) = mpsc::channel(1);
    let (entered_tx, entered_rx) = oneshot::channel();
    let (release_tx, release_rx) = blocking_mpsc::channel();
    let mut entered_tx = Some(entered_tx);
    let handled = Arc::new(AtomicUsize::new(0));
    let handled_worker = handled.clone();
    supervisor.monitor(
        start(
            Duration::from_secs(60),
            receiver,
            shutdown.clone(),
            move |work| {
                if let Work::Incoming(owned_message) = work {
                    handled_worker.fetch_add(1, Ordering::SeqCst);
                    entered_tx
                        .take()
                        .expect("shutdown must prevent a second work item")
                        .send(())
                        .unwrap();
                    release_rx
                        .recv_timeout(Duration::from_secs(2))
                        .expect("test releases physical work");
                    drop(owned_message);
                }
            },
        )
        .expect("valid multithreaded gossip worker"),
    );
    let (first, transport_count) =
        RetainedGossip::with_count_for_test(Arc::clone(message().payload()));
    let first_weak = Arc::downgrade(first.payload());
    sender.send(first).await.unwrap();
    let mut supervision = tokio::spawn(supervisor.start());
    timeout(Duration::from_secs(1), entered_rx)
        .await
        .unwrap()
        .unwrap();
    sender.send(message()).await.unwrap();
    shutdown.send();
    assert!(
        timeout(Duration::from_millis(50), &mut supervision)
            .await
            .is_err(),
        "supervision must not finish while its physical operation runs"
    );
    assert!(
        first_weak.upgrade().is_some(),
        "message owner survives shutdown"
    );
    assert_eq!(
        transport_count.available_permits(),
        0,
        "transport credit survives queued and active work"
    );
    release_tx.send(()).unwrap();
    timeout(Duration::from_secs(3), supervision)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(first_weak.upgrade().is_none());
    assert_eq!(
        transport_count.available_permits(),
        1,
        "physical return releases exact credit"
    );
    assert_eq!(handled.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn already_signalled_shutdown_wins_over_ready_tick_and_message() {
    let mut supervisor = Supervisor::new();
    let shutdown = supervisor.shutdown_signal();
    let (sender, receiver) = mpsc::channel(1);
    sender.send(message()).await.unwrap();
    shutdown.send();
    supervisor.monitor(
        start(Duration::from_secs(60), receiver, shutdown, |_| {
            panic!("ready work must not start after cooperative shutdown");
        })
        .expect("valid multithreaded gossip worker"),
    );
    timeout(Duration::from_secs(1), supervisor.start())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn physical_worker_panic_remains_visible_to_supervisor() {
    let mut supervisor = Supervisor::new();
    let shutdown = supervisor.shutdown_signal();
    let (_sender, receiver) = mpsc::channel(1);
    supervisor.monitor(
        start(Duration::from_secs(60), receiver, shutdown.clone(), |_| {
            // The deadline covers unwind propagation and supervision. Inject the
            // unwind directly so process-global panic-hook I/O and backtrace
            // symbolization cannot consume that protocol-observation budget.
            std::panic::resume_unwind(Box::new("deliberate physical gossip worker failure"));
        })
        .expect("valid multithreaded gossip worker"),
    );
    let result = timeout(Duration::from_secs(1), supervisor.start())
        .await
        .unwrap();
    let report = result.expect_err("the physical worker panic must reach supervision");
    let mut contexts = report.current_contexts();
    assert!(matches!(
        contexts.next(),
        Some(SupervisorError::ChildPanicked)
    ));
    assert!(
        contexts.next().is_none(),
        "only the deliberate panic is reported"
    );
    assert!(shutdown.is_sent());
}

#[tokio::test(flavor = "current_thread")]
async fn unsupported_runtime_returns_error_before_publishing_actor() {
    let (_sender, receiver) = mpsc::channel(1);
    let result = start(
        Duration::from_secs(60),
        receiver,
        ShutdownSignal::new(),
        |_| {},
    );
    assert!(matches!(
        result,
        Err(TransactionGossiperStartError::UnsupportedRuntime)
    ));
}

#[test]
fn missing_runtime_returns_error_before_publishing_actor() {
    let (_sender, receiver) = mpsc::channel(1);
    let result = start(
        Duration::from_secs(60),
        receiver,
        ShutdownSignal::new(),
        |_| {},
    );
    assert!(matches!(
        result,
        Err(TransactionGossiperStartError::MissingRuntime)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn zero_period_returns_error_before_publishing_actor() {
    let (_sender, receiver) = mpsc::channel(1);
    let result = start(Duration::ZERO, receiver, ShutdownSignal::new(), |_| {});
    assert!(matches!(
        result,
        Err(TransactionGossiperStartError::ZeroPeriod)
    ));
}

fn deferred_message(
    message: RetainedGossip<Arc<TransactionGossip>>,
    deadline: tokio::time::Instant,
) -> PendingGossip {
    let mut progress = crate::gossiper::GossipProgress::default();
    progress.defer(0, 1);
    PendingGossip {
        message,
        progress,
        deadline,
        required_height: 1,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn publication_wait_retains_active_and_queued_owners_while_ticks_progress() {
    let mut supervisor = Supervisor::new();
    let shutdown = supervisor.shutdown_signal();
    let (sender, receiver) = mpsc::channel(1);
    let (entered_tx, entered_rx) = oneshot::channel();
    let (tick_tx, tick_rx) = oneshot::channel();
    let mut entered_tx = Some(entered_tx);
    let mut tick_tx = Some(tick_tx);
    let frontier = Arc::new(AtomicUsize::new(0));
    let notify = Arc::new(tokio::sync::Notify::new());
    let wait_frontier = frontier.clone();
    let wait_notify = notify.clone();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    let incoming = Arc::new(AtomicUsize::new(0));
    let handled = incoming.clone();
    let stop = shutdown.clone();
    let mut deferred = false;
    supervisor.monitor(
        super::start(
            Duration::from_millis(10),
            receiver,
            shutdown,
            move |height| {
                let frontier = wait_frontier.clone();
                let notify = wait_notify.clone();
                async move {
                    loop {
                        let changed = notify.notified();
                        tokio::pin!(changed);
                        changed.as_mut().enable();
                        if frontier.load(Ordering::Acquire) >= height as usize {
                            break;
                        }
                        changed.await;
                    }
                }
            },
            move |work| match work {
                Work::Tick => {
                    if deferred && let Some(tick) = tick_tx.take() {
                        tick.send(()).unwrap();
                    }
                    None
                }
                Work::Incoming(message) => {
                    assert_eq!(
                        handled.fetch_add(1, Ordering::SeqCst),
                        0,
                        "the active slot must block dequeue of the second input"
                    );
                    deferred = true;
                    entered_tx.take().unwrap().send(()).unwrap();
                    Some(deferred_message(message, deadline))
                }
                Work::Retry(pending) => {
                    assert_eq!(
                        pending.deadline, deadline,
                        "retry cannot renew its residence budget"
                    );
                    drop(pending);
                    stop.send();
                    None
                }
            },
        )
        .unwrap(),
    );
    let (first, first_count) = RetainedGossip::with_count_for_test(Arc::clone(message().payload()));
    let (second, second_count) =
        RetainedGossip::with_count_for_test(Arc::clone(message().payload()));
    sender.send(first).await.unwrap();
    let supervision = tokio::spawn(supervisor.start());
    timeout(Duration::from_secs(1), entered_rx)
        .await
        .unwrap()
        .unwrap();
    sender.send(second).await.unwrap();
    timeout(Duration::from_secs(1), tick_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first_count.available_permits(), 0);
    assert_eq!(second_count.available_permits(), 0);
    frontier.store(1, Ordering::Release);
    notify.notify_waiters();
    timeout(Duration::from_secs(2), supervision)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(incoming.load(Ordering::SeqCst), 1);
    assert_eq!(first_count.available_permits(), 1);
    assert_eq!(second_count.available_permits(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pending_deadline_expires_without_another_physical_attempt() {
    let mut supervisor = Supervisor::new();
    let shutdown = supervisor.shutdown_signal();
    let (sender, receiver) = mpsc::channel(1);
    let (entered_tx, entered_rx) = oneshot::channel();
    let mut entered_tx = Some(entered_tx);
    let retries = Arc::new(AtomicUsize::new(0));
    let retry_count = retries.clone();
    supervisor.monitor(
        super::start(
            Duration::from_secs(60),
            receiver,
            shutdown.clone(),
            |_| std::future::pending::<()>(),
            move |work| match work {
                Work::Incoming(message) => {
                    entered_tx.take().unwrap().send(()).unwrap();
                    // A completed physical operation can consume the original
                    // deadline; expiration must retire it before any new attempt.
                    Some(deferred_message(message, tokio::time::Instant::now()))
                }
                Work::Retry(_) => {
                    retry_count.fetch_add(1, Ordering::SeqCst);
                    None
                }
                Work::Tick => None,
            },
        )
        .unwrap(),
    );
    let (owned, count) = RetainedGossip::with_count_for_test(Arc::clone(message().payload()));
    sender.send(owned).await.unwrap();
    let supervision = tokio::spawn(supervisor.start());
    timeout(Duration::from_secs(1), entered_rx)
        .await
        .unwrap()
        .unwrap();
    timeout(Duration::from_secs(1), async {
        while count.available_permits() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    shutdown.send();
    timeout(Duration::from_secs(1), supervision)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(retries.load(Ordering::SeqCst), 0);
    assert_eq!(count.available_permits(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn shutdown_drops_pending_publication_without_waiting_for_the_frontier() {
    let mut supervisor = Supervisor::new();
    let shutdown = supervisor.shutdown_signal();
    let (sender, receiver) = mpsc::channel(1);
    let (entered_tx, entered_rx) = oneshot::channel();
    let mut entered_tx = Some(entered_tx);
    supervisor.monitor(
        super::start(
            Duration::from_secs(60),
            receiver,
            shutdown.clone(),
            |_| std::future::pending::<()>(),
            move |work| match work {
                Work::Incoming(message) => {
                    entered_tx.take().unwrap().send(()).unwrap();
                    Some(deferred_message(
                        message,
                        tokio::time::Instant::now() + Duration::from_secs(60),
                    ))
                }
                Work::Retry(_) => panic!("an unpublished frontier must not retry"),
                Work::Tick => None,
            },
        )
        .unwrap(),
    );
    let (owned, count) = RetainedGossip::with_count_for_test(Arc::clone(message().payload()));
    sender.send(owned).await.unwrap();
    let supervision = tokio::spawn(supervisor.start());
    timeout(Duration::from_secs(1), entered_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(count.available_permits(), 0);
    shutdown.send();
    timeout(Duration::from_secs(1), supervision)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(count.available_permits(), 1);
}
