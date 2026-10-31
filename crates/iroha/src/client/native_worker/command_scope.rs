//! One command's completion frontier over the existing canonical native worker.

use std::sync::{Arc, Mutex};

use tokio::sync::Notify;

#[derive(Default)]
struct State {
    jobs: usize,
    command_finished: bool,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    changed: Notify,
}

impl Shared {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

tokio::task_local! {
    static CURRENT_COMMAND: Arc<Shared>;
}

/// The unique execution half of one native command cleanup scope.
///
/// Consume this owner with [`Self::run`] in the command task. Its paired drain owner stays
/// with the supervisor, outside that task, so a command panic cannot discard cleanup evidence.
/// This scope observes calls to the canonical native worker in this task; spawned tasks do not
/// inherit it. A provider that starts separate tasks must own and join those tasks itself.
/// This is cleanup accounting only, not execution admission or monetary authority.
pub struct NativeWorkerCommand {
    shared: Arc<Shared>,
}

/// The supervisor's cleanup frontier for exactly one original native command.
///
/// Keep this owner outside the command task and wait after that task actually joins, including
/// a panic or cancellation. Neither task completion nor a dropped async worker receiver proves
/// that an admitted native worker has completed its original captures, result cleanup and TLS.
/// The host must keep its Tokio runtime alive until this frontier completes.
pub struct NativeWorkerCommandDrain {
    shared: Arc<Shared>,
}

/// Create a unique command scope and its independent cleanup frontier.
///
/// No scope can be reused for another command, and neither owner grants capacity or starts work.
/// The execution half is consumed once; dropping it without polling also finishes that command.
#[must_use]
pub fn native_worker_command_scope() -> (NativeWorkerCommand, NativeWorkerCommandDrain) {
    let shared = Arc::new(Shared::default());
    (
        NativeWorkerCommand {
            shared: shared.clone(),
        },
        NativeWorkerCommandDrain { shared },
    )
}

impl NativeWorkerCommand {
    /// Poll an original command while registering its canonical native jobs with this scope.
    ///
    /// The factory is invoked inside the scope, so a directly returned native-worker future
    /// registers before its eager capture construction. Do not preconstruct worker futures
    /// outside this factory. An entirely unpolled command never invokes its factory.
    ///
    /// Panics keep their usual task semantics. The supervisor must join the task and then await
    /// the paired drain; it must not abort or discard accepted work on a caller's cancellation.
    pub fn run<F: Future>(
        self,
        make_future: impl FnOnce() -> F,
    ) -> impl Future<Output = F::Output> {
        let shared = self.shared.clone();
        async move {
            let result = CURRENT_COMMAND
                .scope(shared, async move { make_future().await })
                .await;
            drop(self);
            result
        }
    }
}

impl Drop for NativeWorkerCommand {
    fn drop(&mut self) {
        self.shared.state().command_finished = true;
        self.shared.changed.notify_waiters();
    }
}

impl NativeWorkerCommandDrain {
    /// Wait until the original command and all its registered native jobs have really ended.
    ///
    /// Cancelling this wait does not discard the frontier or any worker. Calling it again is
    /// safe. No timeout implies permission to close a session or destroy its host runtime.
    pub async fn wait(&self) {
        loop {
            let changed = self.shared.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            {
                let state = self.shared.state();
                if state.command_finished && state.jobs == 0 {
                    return;
                }
            }
            changed.await;
        }
    }
}

// This must be the LAST field of WorkerJob: field cleanup follows its Drop implementation.
// An unstarted job, native panic, successful transfer and cancelled result share this end.
pub(super) struct NativeWorkerRegistration {
    shared: Arc<Shared>,
}

pub(super) fn register() -> Option<NativeWorkerRegistration> {
    CURRENT_COMMAND
        .try_with(|shared| {
            let mut state = shared.state();
            assert!(!state.command_finished, "original native command is active");
            state.jobs = state.jobs.checked_add(1).expect("bounded native jobs");
            NativeWorkerRegistration {
                shared: shared.clone(),
            }
        })
        .ok()
}

impl Drop for NativeWorkerRegistration {
    fn drop(&mut self) {
        let mut state = self.shared.state();
        state.jobs = state.jobs.checked_sub(1).expect("original native job");
        drop(state);
        self.shared.changed.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{native_worker::WorkerJob, run_native_worker};
    use std::{cell::RefCell, sync::mpsc, time::Duration};
    use tokio::sync::{Semaphore, oneshot};

    struct HeldDrop {
        entered: Option<oneshot::Sender<()>>,
        release: mpsc::Receiver<()>,
    }
    impl Drop for HeldDrop {
        fn drop(&mut self) {
            let _ = self.entered.take().unwrap().send(());
            self.release.recv().unwrap();
        }
    }
    thread_local! {
        static HELD_TLS: RefCell<Option<HeldDrop>> = const { RefCell::new(None) };
    }

    #[tokio::test(flavor = "current_thread")]
    async fn command_panic_does_not_complete_frontier_before_original_native_tls() {
        let (scope, drain) = native_worker_command_scope();
        let permits = Arc::new(Semaphore::new(1));
        let permit = permits.clone().try_acquire_owned().unwrap();
        let (entered, entered_rx) = oneshot::channel();
        let (release, release_rx) = mpsc::channel();
        let command = tokio::spawn(scope.run(|| async move {
            let work = run_native_worker(permit, (), move || {
                HELD_TLS.with(|slot| {
                    *slot.borrow_mut() = Some(HeldDrop {
                        entered: Some(entered),
                        release: release_rx,
                    });
                });
            });
            tokio::pin!(work);
            tokio::select! {
                result = &mut work => panic!("native TLS returned early: {result:?}"),
                result = entered_rx => result.unwrap(),
            }
            panic!("original command panic while its native TLS is held");
        }));
        assert!(command.await.unwrap_err().is_panic());
        assert_eq!(permits.available_permits(), 0);
        assert!(
            tokio::time::timeout(Duration::from_millis(20), drain.wait())
                .await
                .is_err()
        );
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), drain.wait())
            .await
            .unwrap();
        assert_eq!(permits.available_permits(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ready_cancelled_result_cleanup_remains_inside_original_command_frontier() {
        let (scope, drain) = native_worker_command_scope();
        let permits = Arc::new(Semaphore::new(1));
        let permit = permits.clone().try_acquire_owned().unwrap();
        let (entered, entered_rx) = oneshot::channel();
        let (release, release_rx) = mpsc::channel();
        let command = tokio::spawn(scope.run(|| async move {
            let (job, receiver, ready) = WorkerJob::new(permit, (), move || HeldDrop {
                entered: Some(entered),
                release: release_rx,
            });
            let _join = tokio::task::spawn_blocking(move || job.run());
            ready.await.unwrap();
            // Delivery happened, but the receiver never claimed the result.
            drop(receiver);
        }));
        command.await.unwrap();
        entered_rx.await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), drain.wait())
                .await
                .is_err()
        );
        assert_eq!(permits.available_permits(), 0);
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), drain.wait())
            .await
            .unwrap();
        assert_eq!(permits.available_permits(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn separate_and_unpolled_commands_never_borrow_another_frontier() {
        let (first, first_drain) = native_worker_command_scope();
        let (second, second_drain) = native_worker_command_scope();
        let permits = Arc::new(Semaphore::new(1));
        let permit = permits.clone().try_acquire_owned().unwrap();
        let (entered, entered_rx) = oneshot::channel();
        let (release, release_rx) = mpsc::channel();
        let task = tokio::spawn(second.run(|| async move {
            run_native_worker(permit, (), move || {
                let _held = HeldDrop {
                    entered: Some(entered),
                    release: release_rx,
                };
            })
            .await
            .unwrap();
        }));
        entered_rx.await.unwrap();
        let unpolled = first.run(std::future::pending::<()>);
        drop(unpolled);
        tokio::time::timeout(Duration::from_secs(5), first_drain.wait())
            .await
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), second_drain.wait())
                .await
                .is_err()
        );
        release.send(()).unwrap();
        task.await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), second_drain.wait())
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "current_thread")]
    async fn direct_worker_factory_registers_before_eager_capture_and_unpolled_factory_is_idle() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let (unpolled, unpolled_drain) = native_worker_command_scope();
        let invoked = Arc::new(AtomicBool::new(false));
        let observed = invoked.clone();
        let future = unpolled.run(move || {
            observed.store(true, Ordering::SeqCst);
            std::future::ready(())
        });
        drop(future);
        unpolled_drain.wait().await;
        assert!(!invoked.load(Ordering::SeqCst));

        let (scope, drain) = native_worker_command_scope();
        let permits = Arc::new(Semaphore::new(1));
        let permit = permits.clone().try_acquire_owned().unwrap();
        let (entered, entered_rx) = oneshot::channel();
        let (release, release_rx) = mpsc::channel();
        let command = tokio::spawn(scope.run(move || {
            run_native_worker(permit, (), move || {
                HELD_TLS.with(|slot| {
                    *slot.borrow_mut() = Some(HeldDrop {
                        entered: Some(entered),
                        release: release_rx,
                    });
                });
            })
        }));
        entered_rx.await.unwrap();
        // Internal test fault only; the session API never exposes a command abort handle.
        command.abort();
        assert!(command.await.unwrap_err().is_cancelled());
        assert!(
            tokio::time::timeout(Duration::from_millis(20), drain.wait())
                .await
                .is_err()
        );
        assert_eq!(permits.available_permits(), 0);
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(5), drain.wait())
            .await
            .unwrap();
        assert_eq!(permits.available_permits(), 1);
    }
}
