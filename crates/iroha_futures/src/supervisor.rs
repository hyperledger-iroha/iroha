//! Lightweight supervisor for tokio tasks.
//!
//! What it does:
//!
//! - Monitors multiple children (as spawned [`JoinHandle`])
//! - Provides a single shutdown signal for everything
//! - Supports graceful shutdown timeout before aborting a child (via [`OnShutdown`])
//! - If a child panics, initiates shutdown and exits with an error
//! - If a child exits before shutdown signal, also initiates shutdown and exits with an error.
//!   Note: this might not be always the desirable behaviour, but _currently_ there are no other
//!   cases in Iroha.
//!   This behaviour could be easily extended to support refined strategies.
//! - Logs children's lifecycle
//!
//! What it doesn't:
//!
//! - Doesn't support restarting child.
//!   To implement that, we need a formal actor system, i.e. actors with a unified lifecycle,
//!   messaging system, and, most importantly, address registry
//!   (for reference, see [Registry - Elixir](https://hexdocs.pm/elixir/1.17.0-rc.1/Registry.html)).

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use error_stack::Report;
use iroha_logger::{InstrumentFutures, prelude::Span};
use tokio::{
    sync::{Notify, mpsc, oneshot},
    task::{JoinHandle, JoinSet},
    time::timeout,
};
use tokio_util::sync::CancellationToken;

/// Supervisor for tokio tasks.
#[derive(Debug, Default)]
pub struct Supervisor {
    children: Vec<Child>,
    shutdown_signal: ShutdownSignal,
}

impl Supervisor {
    /// Constructor
    pub fn new() -> Self {
        Self::default()
    }

    /// Get a copy of the supervisor's shutdown signal
    pub fn shutdown_signal(&self) -> ShutdownSignal {
        self.shutdown_signal.clone()
    }

    /// Monitor a given [`Child`]
    #[track_caller]
    pub fn monitor(&mut self, child: impl Into<Child>) {
        self.children.push(child.into());
    }

    /// Spawns a task that will initiate supervisor shutdown on SIGINT/SIGTERM signals.
    /// # Errors
    /// See [`tokio::signal::unix::signal`] errors.
    pub fn setup_shutdown_on_os_signals(&mut self) -> Result<()> {
        use tokio::signal;

        let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
            .map_err(|err| Report::new(Error::from(err)).expand())?;
        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
            .map_err(|err| Report::new(Error::from(err)).expand())?;

        let shutdown_signal = self.shutdown_signal();
        self.monitor(tokio::spawn(async move {
            tokio::select! {
                _ = sigint.recv() => {
                    iroha_logger::info!("SIGINT received, shutting down...");
                },
                _ = sigterm.recv() => {
                    iroha_logger::info!("SIGTERM received, shutting down...");
                },
            }

            shutdown_signal.send();
        }));

        Ok(())
    }

    /// Spawns a task that will shut down the supervisor once the external
    /// [`ShutdownSignal`] is sent.
    pub fn shutdown_on_external_signal(&mut self, external_signal: ShutdownSignal) {
        let self_signal = self.shutdown_signal();

        self.monitor(tokio::spawn(async move {
            external_signal.receive().await;
            self_signal.send();
        }))
    }

    /// Start actual supervision and wait until all children terminate.
    ///
    /// Returns [`Ok`] if all children exited/aborted as expected after shutdown
    /// signal being sent.
    ///
    /// # Errors
    /// If any child panicked during execution or exited/aborted before shutdown signal being sent.
    pub async fn start(self) -> Result<()> {
        // technically - should work without this check too
        if self.children.is_empty() {
            return Ok(());
        }

        LoopBuilder::new(self.shutdown_signal)
            .monitor(self.children)
            .into_loop()
            .run()
            .await
    }
}

#[derive(Clone, Debug, Default)]
struct NotifyOnce {
    notify: Arc<Notify>,
    is_notified: Arc<AtomicBool>,
}

impl NotifyOnce {
    fn new() -> Self {
        Self {
            notify: Arc::new(Notify::new()),
            is_notified: Arc::new(AtomicBool::new(false)),
        }
    }

    fn notify(&self) {
        self.is_notified.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }

    async fn notified(&self) {
        if !self.is_notified.load(Ordering::Acquire) {
            self.notify.notified().await;
        }
    }
}

struct LoopBuilder {
    set: JoinSet<()>,
    shutdown_signal: ShutdownSignal,
    exit: (
        mpsc::Sender<ChildExitResult>,
        mpsc::Receiver<ChildExitResult>,
    ),
}

impl LoopBuilder {
    fn new(shutdown_signal: ShutdownSignal) -> Self {
        Self {
            set: JoinSet::new(),
            shutdown_signal,
            exit: mpsc::channel(256),
        }
    }

    fn monitor(mut self, children: impl IntoIterator<Item = Child>) -> Self {
        for child in children {
            self.monitor_single(child);
        }
        self
    }

    fn monitor_single(
        &mut self,
        Child {
            task,
            on_shutdown,
            span,
        }: Child,
    ) {
        let exit_tx = self.exit.0.clone();

        let exit_signal = NotifyOnce::new();
        let exit_signal_task = exit_signal.clone();
        let abort_handle = self.set.spawn(
            async move {
                iroha_logger::debug!("Start monitoring a child");
                let result = match task.await {
                    Ok(()) => {
                        iroha_logger::debug!("Child exited normally");
                        ChildExitResult::Ok
                    }
                    Err(err) if err.is_panic() => {
                        // we could call `err.into_panic()` and downcast `Any` to `String`,
                        // but the panic message was most probably already printed
                        iroha_logger::error!("Child panicked");
                        ChildExitResult::Panic
                    }
                    Err(err) if err.is_cancelled() => {
                        iroha_logger::debug!("Child aborted"); // oh..
                        ChildExitResult::Cancel
                    }
                    _ => unreachable!(),
                };
                let _ = exit_tx.send(result).await;
                exit_signal_task.notify();
            }
            .instrument(span.clone()),
        );

        // task to handle graceful shutdown
        let shutdown_signal = self.shutdown_signal.clone();
        let exit_wait1 = exit_signal.clone();
        let exit_wait2 = exit_signal.clone();
        self.set.spawn(async move {
            tokio::select! {
               () = exit_wait1.notified() => {
                   // exit
               }
               () =  shutdown_signal.receive() => {
                   match on_shutdown {
                        OnShutdown::Abort => {
                            iroha_logger::debug!("Shutdown signal received, aborting...");
                            abort_handle.abort();
                        }
                        OnShutdown::Wait(duration) => {
                            iroha_logger::debug!(?duration, "Shutdown signal received, waiting for child shutdown...");
                            if timeout(duration, exit_wait2.notified()).await.is_err() {
                                iroha_logger::debug!(expected = ?duration, "Child shutdown took longer than expected, aborting...");
                                abort_handle.abort();
                            }
                        }
                    }

               }
           }
        }.instrument(span));
    }

    fn into_loop(self) -> SupervisorLoop {
        let Self {
            set,
            shutdown_signal,
            exit: (_tx, exit_rx),
        } = self;
        SupervisorLoop {
            set,
            shutdown_signal,
            exit: exit_rx,
            caught_panic: false,
            caught_unexpected_exit: false,
        }
    }
}

struct SupervisorLoop {
    set: JoinSet<()>,
    shutdown_signal: ShutdownSignal,
    exit: mpsc::Receiver<ChildExitResult>,
    caught_panic: bool,
    caught_unexpected_exit: bool,
}

impl SupervisorLoop {
    async fn run(mut self) -> Result<()> {
        loop {
            tokio::select! {
                // this should naturally finish when all supervisor-spawned tasks finish
                Some(result) = self.set.join_next() => {
                    if let Err(err) = result
                        && err.is_panic()
                    {
                        iroha_logger::error!(?err, "Supervisor-spawned task panicked; it is probably a bug");
                    }
                }

                // this should finish when all task monitors finish
                Some(result) = self.exit.recv() => {
                    iroha_logger::debug!(?result, "Child exited");
                    self.handle_child_exit(result);
                }

                else => break,
            }
        }

        let mut report: Option<Report<[Error]>> = if self.caught_panic {
            Some(Report::new(Error::ChildPanicked).expand())
        } else {
            None
        };
        if self.caught_unexpected_exit {
            match &mut report {
                Some(r) => r.push(Report::new(Error::UnexpectedExit)),
                None => report = Some(Report::new(Error::UnexpectedExit).expand()),
            }
        }
        report.map_or(Ok(()), Err)
    }

    fn handle_child_exit(&mut self, result: ChildExitResult) {
        iroha_logger::warn!(
            ?result,
            shutdown_sent = self.shutdown_signal.is_sent(),
            "Supervisor observed child exit"
        );
        match result {
            ChildExitResult::Ok | ChildExitResult::Cancel if !self.shutdown_signal.is_sent() => {
                iroha_logger::error!("Some task exited unexpectedly, shutting down everything...");
                self.caught_unexpected_exit = true;
                self.shutdown_signal.send();
            }
            ChildExitResult::Panic if !self.caught_panic => {
                iroha_logger::error!("Some task panicked, shutting down everything...");
                self.caught_panic = true;
                self.shutdown_signal.send();
            }
            _ => {}
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum ChildExitResult {
    Ok,
    Panic,
    Cancel,
}

/// Signal indicating system shutdown. Could be cloned around.
///
/// It is effectively a wrap around [`CancellationToken`], but with different naming.
#[derive(Clone, Debug, Default)]
pub struct ShutdownSignal(CancellationToken);

impl ShutdownSignal {
    /// Constructor
    pub fn new() -> Self {
        Self::default()
    }

    /// Send the shutdown signal, resolving all [`Self::receive`] futures.
    pub fn send(&self) {
        self.0.cancel();
    }

    /// Receive the shutdown signal. Resolves after [`Self::send`].
    pub async fn receive(&self) {
        self.0.cancelled().await
    }

    /// Sync check whether the shutdown signal was sent
    pub fn is_sent(&self) -> bool {
        self.0.is_cancelled()
    }
}

/// Spawn [`std::thread`] as a future that finishes when the thread finishes and panics
/// when the thread panics.
///
/// Its intention is to link an OS thread to [`Supervisor`] in the following way:
///
/// ```
/// use std::time::Duration;
///
/// use iroha_futures::supervisor::{
///     Child, OnShutdown, ShutdownSignal, Supervisor, spawn_os_thread_as_future,
/// };
///
/// fn spawn_heavy_work(shutdown_signal: ShutdownSignal) -> Child {
///     Child::new(
///         tokio::spawn(spawn_os_thread_as_future(
///             std::thread::Builder::new().name("heavy_worker".to_owned()),
///             move || {
///                 loop {
///                     if shutdown_signal.is_sent() {
///                         break;
///                     }
///                     // do heavy work...
///                     std::thread::sleep(Duration::from_millis(100));
///                 }
///             },
///         )),
///         OnShutdown::Wait(Duration::from_secs(1)),
///     )
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let mut supervisor = Supervisor::new();
///     supervisor.monitor(spawn_heavy_work(supervisor.shutdown_signal()));
///
///     let signal = supervisor.shutdown_signal();
///     tokio::spawn(async move {
///         tokio::time::sleep(Duration::from_millis(300)).await;
///         signal.send();
///     });
///
///     supervisor.start().await.unwrap();
/// }
/// ```
///
/// **Note:** this function doesn't provide a mechanism to shut down the thread.
/// You should handle it within the closure on your own, e.g. by passing [`ShutdownSignal`] inside.
pub async fn spawn_os_thread_as_future<F>(builder: std::thread::Builder, f: F)
where
    F: FnOnce(),
    F: Send + 'static,
{
    let (complete_tx, complete_rx) = oneshot::channel();

    // we are okay to drop the handle; thread will continue running in a detached way
    let _handle: std::thread::JoinHandle<_> = builder
        .spawn(move || {
            f();

            // the receiver might be dropped
            let _ = complete_tx.send(());
        })
        .expect("should spawn thread normally");

    complete_rx
        .await
        .expect("thread completion notifier was dropped; thread probably panicked");
}

/// Supervisor child.
#[derive(Debug)]
pub struct Child {
    span: Span,
    task: JoinHandle<()>,
    on_shutdown: OnShutdown,
}

impl Child {
    /// Create a new supervisor child
    #[track_caller]
    pub fn new(task: JoinHandle<()>, on_shutdown: OnShutdown) -> Self {
        let caller_location = std::panic::Location::caller().to_string();
        let span = iroha_logger::debug_span!("supervisor_child_monitor", %caller_location);

        Self {
            span,
            task,
            on_shutdown,
        }
    }
}

impl From<JoinHandle<()>> for Child {
    #[track_caller]
    fn from(value: JoinHandle<()>) -> Self {
        Self::new(value, OnShutdown::Abort)
    }
}

/// Supervisor errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// One or more supervised tasks panicked.
    #[error("Some of the supervisor children panicked")]
    ChildPanicked,
    /// One or more supervised tasks exited unexpectedly.
    #[error("Some of the supervisor children exited unexpectedly")]
    UnexpectedExit,
    /// I/O error propagated from child tasks.
    #[error("IO error")]
    IO(#[from] std::io::Error),
}

/// Result type returned by supervisor methods.
pub type Result<T> = core::result::Result<T, Report<[Error]>>;

/// Specifies supervisor action regarding a [`Child`] when shutdown happens.
#[derive(Default, Copy, Clone, Debug)]
pub enum OnShutdown {
    /// Abort the child immediately
    #[default]
    Abort,
    /// Wait until the child exits/aborts on its own; abort if it takes too long
    Wait(Duration),
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    use tokio::{
        sync::{mpsc, oneshot},
        time::sleep,
    };

    use super::*;

    const TICK_TIMEOUT: Duration = Duration::from_millis(10);
    /// For some reason, when all tests are run simultaneously, tests with OS spawns take longer
    /// than just [`TICK_TIMEOUT`]
    const OS_THREAD_SPAWN_TICK: Duration = Duration::from_millis(500);
    const SHUTDOWN_WITHIN_TICK: OnShutdown = OnShutdown::Wait(TICK_TIMEOUT);

    #[tokio::test]
    async fn empty_supervisor_just_exits() {
        timeout(TICK_TIMEOUT, Supervisor::new().start())
            .await
            .expect("should exit immediately")
            .expect("should not emit error");
    }

    #[tokio::test]
    async fn happy_graceful_shutdown() {
        #[derive(Debug)]
        enum Message {
            Ping { pong: oneshot::Sender<()> },
            Stopped,
        }

        let mut sup = Supervisor::new();

        let (tx_into, mut rx_into) = mpsc::channel(1);
        let (tx_out, rx_out) = oneshot::channel();

        {
            let shutdown = sup.shutdown_signal();
            sup.monitor(Child::new(
                tokio::spawn(async move {
                    loop {
                        tokio::select! {
                            Some(Message::Ping { pong }) = rx_into.recv() => {
                                pong.send(()).unwrap();
                            },
                            () = shutdown.receive() => {
                                tx_out.send(Message::Stopped).unwrap();
                                break;
                            }
                        }
                    }
                }),
                SHUTDOWN_WITHIN_TICK,
            ));
        }

        // ensure task is spinning
        timeout(TICK_TIMEOUT, async {
            let (tx, rx) = oneshot::channel();
            tx_into.send(Message::Ping { pong: tx }).await.unwrap();
            rx.await.unwrap();
        })
        .await
        .unwrap();

        let shutdown = sup.shutdown_signal();
        let sup_handle = tokio::spawn(sup.start());

        // send shutdown signal
        shutdown.send();
        timeout(TICK_TIMEOUT, async {
            let Message::Stopped = rx_out.await.unwrap() else {
                panic!("expected stopped message");
            };
        })
        .await
        .unwrap();

        // we can now expect supervisor to stop without errors
        timeout(TICK_TIMEOUT, sup_handle)
            .await
            .unwrap()
            .expect("supervisor run should not panic")
            .expect("supervisor should not find any nested panics");
    }

    #[tokio::test]
    async fn supervisor_catches_panic_of_a_monitored_task() {
        let mut sup = Supervisor::new();

        sup.monitor(tokio::spawn(async {
            panic!("my panic should not be unnoticed")
        }));

        let err = timeout(TICK_TIMEOUT, sup.start())
            .await
            .expect("should finish almost immediately")
            .expect_err("should catch the panic");
        let mut contexts = err.current_contexts();
        let first = contexts.next().expect("at least one context");
        assert!(matches!(first, Error::ChildPanicked));
    }

    #[tokio::test]
    async fn supervisor_sends_shutdown_when_some_task_exits() {
        let mut sup = Supervisor::new();

        // exits immediately, not expected
        sup.monitor(tokio::spawn(async {}));

        // some task that needs shutdown gracefully
        let signal = sup.shutdown_signal();
        let (graceful_tx, graceful_rx) = oneshot::channel();
        sup.monitor(Child::new(
            tokio::spawn(async move {
                signal.receive().await;
                graceful_tx.send(()).unwrap();
            }),
            SHUTDOWN_WITHIN_TICK,
        ));

        let sup_handle = tokio::spawn(sup.start());

        timeout(TICK_TIMEOUT, graceful_rx)
            .await
            .expect("should shutdown everything immediately")
            .expect("should receive message fine");

        let err = timeout(TICK_TIMEOUT, sup_handle)
            .await
            .unwrap()
            .expect("supervisor should not panic")
            .expect_err("should handle unexpected exit");
        assert!(
            err.current_contexts()
                .any(|ctx| matches!(ctx, Error::UnexpectedExit))
        );
    }

    #[tokio::test]
    async fn graceful_shutdown_when_some_task_panics() {
        let mut sup = Supervisor::new();
        let signal = sup.shutdown_signal();
        sup.monitor(tokio::spawn(async { panic!() }));

        let err = timeout(TICK_TIMEOUT, sup.start())
            .await
            .expect("should finish immediately")
            .expect_err("should catch the panic");
        assert!(
            err.current_contexts()
                .any(|ctx| matches!(ctx, Error::ChildPanicked))
        );

        assert!(signal.is_sent());
    }

    fn spawn_task_with_graceful_shutdown(
        sup: &mut Supervisor,
        shutdown_time: Duration,
        timeout: Duration,
    ) -> Arc<AtomicBool> {
        let graceful = Arc::new(AtomicBool::new(false));

        let signal = sup.shutdown_signal();
        let graceful_clone = graceful.clone();
        sup.monitor(Child::new(
            tokio::spawn(async move {
                signal.receive().await;
                sleep(shutdown_time).await;
                graceful_clone.fetch_or(true, Ordering::Relaxed);
            }),
            OnShutdown::Wait(timeout),
        ));

        graceful
    }

    #[tokio::test]
    async fn actually_waits_for_shutdown() {
        const ACTUAL_SHUTDOWN: Duration = Duration::from_millis(50);
        const TIMEOUT: Duration = Duration::from_millis(100);

        let mut sup = Supervisor::new();
        let signal = sup.shutdown_signal();
        let graceful = spawn_task_with_graceful_shutdown(&mut sup, ACTUAL_SHUTDOWN, TIMEOUT);
        let sup_fut = tokio::spawn(sup.start());

        signal.send();
        timeout(ACTUAL_SHUTDOWN + TICK_TIMEOUT, sup_fut)
            .await
            .expect("should finish within this time")
            .expect("supervisor should not panic")
            .expect("supervisor should exit fine");
        assert!(graceful.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn aborts_task_if_shutdown_takes_long() {
        const ACTUAL_SHUTDOWN: Duration = Duration::from_millis(100);
        const TIMEOUT: Duration = Duration::from_millis(50);

        // Start system
        let mut sup = Supervisor::new();
        let signal = sup.shutdown_signal();
        let graceful = spawn_task_with_graceful_shutdown(&mut sup, ACTUAL_SHUTDOWN, TIMEOUT);
        let sup_fut = tokio::spawn(sup.start());

        // Initiate shutdown
        signal.send();
        timeout(TIMEOUT + TICK_TIMEOUT, sup_fut)
            .await
            .expect("should finish within this time")
            .expect("supervisor should not panic")
            .expect("shutdown took too long, but it is not an error");
        assert!(!graceful.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn can_monitor_os_thread_shutdown() {
        const LOOP_SLEEP: Duration = Duration::from_millis(5);
        const TIMEOUT: Duration = Duration::from_millis(50);

        let mut sup = Supervisor::new();
        let signal = sup.shutdown_signal();
        let signal2 = sup.shutdown_signal();
        let ready = NotifyOnce::new();
        let ready2 = ready.clone();
        let graceful = Arc::new(AtomicBool::new(false));
        let graceful2 = graceful.clone();
        sup.monitor(Child::new(
            tokio::spawn(spawn_os_thread_as_future(
                std::thread::Builder::new(),
                move || {
                    ready2.notify();
                    loop {
                        if signal.is_sent() {
                            graceful.fetch_or(true, Ordering::Relaxed);
                            break;
                        }
                        std::thread::sleep(LOOP_SLEEP);
                    }
                },
            )),
            OnShutdown::Wait(TIMEOUT),
        ));
        // need to yield so that it can actually start the thread
        tokio::task::yield_now().await;
        let sup_fut = tokio::spawn(sup.start());

        timeout(OS_THREAD_SPAWN_TICK, ready.notified())
            .await
            .expect("thread should start by now");
        signal2.send();
        // It is too slow in CI sometimes
        timeout(TICK_TIMEOUT * 10, sup_fut)
            .await
            .expect("should shutdown within timeout")
            .expect("should not panic")
            .expect("should shutdown without errors");
        assert!(graceful2.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn can_catch_os_thread_panic() {
        let mut sup = Supervisor::new();
        sup.monitor(tokio::spawn(spawn_os_thread_as_future(
            std::thread::Builder::new(),
            || panic!("oops"),
        )));
        let err = timeout(OS_THREAD_SPAWN_TICK, sup.start())
            .await
            .expect("should terminate immediately")
            .expect_err("should catch panic");
        assert!(
            err.current_contexts()
                .any(|ctx| matches!(ctx, Error::ChildPanicked))
        );
    }

    #[tokio::test]
    async fn aggregates_multiple_errors() {
        let mut sup = Supervisor::new();
        // one child panics and another exits unexpectedly
        sup.monitor(tokio::spawn(async {}));
        sup.monitor(tokio::spawn(async { panic!("boom") }));

        let err = timeout(TICK_TIMEOUT, sup.start())
            .await
            .expect("should terminate quickly")
            .expect_err("should report errors");

        let mut frame_count = 0;
        let mut seen_panic = false;
        let mut seen_exit = false;
        for frame in err.frames() {
            if let Some(ctx) = frame.downcast_ref::<Error>() {
                frame_count += 1;
                match ctx {
                    Error::ChildPanicked => seen_panic = true,
                    Error::UnexpectedExit => seen_exit = true,
                    _ => {}
                }
            }
        }
        assert_eq!(frame_count, 2);
        assert!(seen_panic && seen_exit);
    }
}
