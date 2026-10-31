//! Bounded ownership of original synchronous native work through cancellation and OS-thread exit.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use tokio::sync::{OwnedSemaphorePermit, oneshot};

mod command_scope;
pub use command_scope::{
    NativeWorkerCommand, NativeWorkerCommandDrain, native_worker_command_scope,
};

/// A local native-worker lifecycle failure, never an execution or consensus verdict.
#[derive(Debug, thiserror::Error)]
pub enum NativeWorkerError {
    /// No Tokio runtime owns the admitted join job.
    #[error("native worker requires an active Tokio runtime")]
    RuntimeUnavailable,
    /// Original OS thread creation failed; the captured work was destroyed before refund.
    #[error("native worker thread creation failed")]
    ThreadCreation(#[source] std::io::Error),
    /// Original work unwound; its panic payload and native TLS have already been destroyed.
    #[error("native worker panicked")]
    Panicked,
    /// The original admitted join job was discarded or could not deliver its completion.
    #[error("native worker completion unavailable")]
    CompletionUnavailable,
}

// Field order is deliberate: an unstarted/refused job destroys its captured work
// before refunding capacity, then wakes the receiver after the refund.
struct WorkerJob<T, F, C> {
    work: Option<F>,
    custody: Option<C>,
    permit: Option<OwnedSemaphorePermit>,
    completed: Option<oneshot::Sender<()>>,
    slot: Arc<CompletionSlot<T, C>>,
    _command: Option<command_scope::NativeWorkerRegistration>,
}

// If delivery loses a cancellation race, destroy the original output before its
// permit. Successful delivery explicitly refunds before returning the output.
struct Completion<T, C> {
    output: Option<Result<T, NativeWorkerError>>,
    custody: Option<C>,
    _permit: OwnedSemaphorePermit,
}

struct CompletionState<T, C> {
    output: Option<Completion<T, C>>,
    receiver_alive: bool,
}

struct CompletionSlot<T, C> {
    state: Mutex<CompletionState<T, C>>,
    changed: Condvar,
}

impl<T, C> CompletionSlot<T, C> {
    fn state(&self) -> MutexGuard<'_, CompletionState<T, C>> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

// Cancelling the async receiver only notifies the original admitted join job.
// In particular, no unclaimed T is ever moved into the oneshot channel.
struct CompletionReceiver<T, C>(Arc<CompletionSlot<T, C>>);

impl<T, C> CompletionReceiver<T, C> {
    fn take(&self) -> Option<Completion<T, C>> {
        let mut state = self.0.state();
        let output = state.output.take();
        state.receiver_alive = false;
        self.0.changed.notify_one();
        output
    }

    async fn receive(self, ready: oneshot::Receiver<()>) -> Result<(T, C), NativeWorkerError> {
        ready
            .await
            .map_err(|_| NativeWorkerError::CompletionUnavailable)?;
        let mut completion = self
            .take()
            .ok_or(NativeWorkerError::CompletionUnavailable)?;
        let output = completion
            .output
            .take()
            .expect("original result is taken once");
        let result = output.map(|output| {
            let custody = completion
                .custody
                .take()
                .expect("original custody is taken once");
            (output, custody)
        });
        drop(completion);
        result
    }
}

impl<T, C> Drop for CompletionReceiver<T, C> {
    fn drop(&mut self) {
        self.0.state().receiver_alive = false;
        self.0.changed.notify_one();
    }
}

impl<T, F, C> Drop for WorkerJob<T, F, C> {
    fn drop(&mut self) {
        let remaining = self.slot.state().output.take();
        // No caller-controlled destructor runs while the slot is locked.
        drop(remaining);
    }
}

impl<T: Send + 'static, F: FnOnce() -> T + Send + 'static, C: Send + 'static> WorkerJob<T, F, C> {
    fn new(
        permit: OwnedSemaphorePermit,
        custody: C,
        work: F,
    ) -> (Self, CompletionReceiver<T, C>, oneshot::Receiver<()>) {
        let slot = Arc::new(CompletionSlot {
            state: Mutex::new(CompletionState {
                output: None,
                receiver_alive: true,
            }),
            changed: Condvar::new(),
        });
        let (completed, ready) = oneshot::channel();
        (
            Self {
                work: Some(work),
                custody: Some(custody),
                permit: Some(permit),
                completed: Some(completed),
                slot: slot.clone(),
                _command: command_scope::register(),
            },
            CompletionReceiver(slot),
            ready,
        )
    }

    fn run(self) {
        self.run_with_spawn(|work| {
            std::thread::Builder::new()
                .name("iroha-native-worker".into())
                .spawn(work)
        });
    }

    fn run_with_spawn(self, spawn: impl FnOnce(F) -> std::io::Result<std::thread::JoinHandle<T>>) {
        self.run_with_spawn_and_ready(spawn, || {});
    }

    fn run_with_spawn_and_ready(
        mut self,
        spawn: impl FnOnce(F) -> std::io::Result<std::thread::JoinHandle<T>>,
        after_ready: impl FnOnce(),
    ) {
        let work = self.work.take().expect("original work is taken once");
        let output = match spawn(work) {
            Ok(worker) => match worker.join() {
                Ok(output) => Ok(output),
                Err(payload) => {
                    // Arbitrary original panic payload destructors also remain
                    // inside this admitted job's lifetime.
                    drop(payload);
                    Err(NativeWorkerError::Panicked)
                }
            },
            Err(error) => Err(NativeWorkerError::ThreadCreation(error)),
        };
        let custody = if output.is_ok() {
            self.custody.take()
        } else {
            // A failed native worker cannot hand custody back. Dispose it on
            // this join job after native TLS/panic cleanup, before refund.
            drop(self.custody.take());
            None
        };
        let completion = Completion {
            output: Some(output),
            custody,
            _permit: self.permit.take().expect("original permit is taken once"),
        };
        self.slot.state().output = Some(completion);
        let sent = self
            .completed
            .take()
            .expect("original sender is taken once")
            .send(())
            .is_ok();
        after_ready();
        if sent {
            let mut state = self.slot.state();
            while state.receiver_alive {
                state = self
                    .slot
                    .changed
                    .wait(state)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        }
        // Self's destructor cleans an unclaimed output on this join job, outside
        // the mutex, even when the ready notification was already delivered.
    }
}

/// Run bounded synchronous native work outside Tokio's inherited blocking context.
///
/// An admitted Tokio blocking job creates and joins the native worker, retaining
/// its original permit through OS-thread exit, including thread-local destruction.
/// Dropping the awaiting future neither cancels that work nor releases capacity
/// early. A cancelled output or original panic payload is destroyed on that job
/// before refund. The async receiver only signals cancellation; successful receipt
/// synchronously claims the original result and releases its permit.
/// No join blocks the async executor and no replacement worker is started.
/// `custody` remains outside the work closure, so unwinding cannot discard an
/// original lease before native TLS exits. Success returns that exact custody
/// together with the result; a failed worker destroys it on the join job.
/// Callers without retained custody pass `()`.
///
/// This operation requires a Tokio runtime. Runtime shutdown may wait for an
/// already-started original worker, just as it waits for other blocking jobs.
/// Finish or drop this future before shutting down its runtime: an unpolled ready
/// receiver deliberately retains its original admitted result until then.
///
/// # Errors
/// Returns the original typed [`NativeWorkerError`] for unavailable runtime, thread
/// creation failure, worker unwind, or unavailable completion. Thread creation preserves
/// its original I/O cause; panic payloads are destroyed while original custody is held.
pub fn run_native_worker<T: Send + 'static, C: Send + 'static>(
    permit: OwnedSemaphorePermit,
    custody: C,
    work: impl FnOnce() -> T + Send + 'static,
) -> impl Future<Output = Result<(T, C), NativeWorkerError>> + Send {
    // Construct custody before returning a future: even an entirely unpolled
    // future drops captured work before its original custody and admission permit.
    let (job, receiver, ready) = WorkerJob::new(permit, custody, work);
    async move {
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| NativeWorkerError::RuntimeUnavailable)?;
        // The original semaphore bounds queued as well as running join jobs.
        // A stopped runtime may discard an unstarted job; no std thread exists
        // until this closure starts, and a running blocking job is not aborted.
        let _join = runtime.spawn_blocking(move || job.run());
        receiver.receive(ready).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::RefCell,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    #[test]
    fn native_worker_creation_error_retains_original_io_cause() {
        let original = std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "original thread refusal",
        );
        let failure = NativeWorkerError::ThreadCreation(original);
        let source = std::error::Error::source(&failure).unwrap();
        assert_eq!(
            source.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        for failure in [
            NativeWorkerError::RuntimeUnavailable,
            NativeWorkerError::Panicked,
            NativeWorkerError::CompletionUnavailable,
        ] {
            assert!(std::error::Error::source(&failure).is_none());
        }
    }

    struct ObservedDrop {
        permits: Arc<tokio::sync::Semaphore>,
        available_on_drop: Arc<AtomicUsize>,
    }

    impl Drop for ObservedDrop {
        fn drop(&mut self) {
            self.available_on_drop
                .store(self.permits.available_permits(), Ordering::SeqCst);
        }
    }

    fn observed_capture(permits: &Arc<tokio::sync::Semaphore>) -> (ObservedDrop, Arc<AtomicUsize>) {
        let available_on_drop = Arc::new(AtomicUsize::new(usize::MAX));
        (
            ObservedDrop {
                permits: permits.clone(),
                available_on_drop: available_on_drop.clone(),
            },
            available_on_drop,
        )
    }

    struct HeldDrop {
        entered: Option<tokio::sync::oneshot::Sender<()>>,
        release: std::sync::mpsc::Receiver<()>,
    }

    impl Drop for HeldDrop {
        fn drop(&mut self) {
            let _ = self.entered.take().unwrap().send(());
            let _ = self.release.recv();
        }
    }

    thread_local! {
        static WORKER_TLS: RefCell<Option<HeldDrop>> = const { RefCell::new(None) };
    }

    async fn returned_capacity(permits: &tokio::sync::Semaphore) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while permits.available_permits() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("original worker eventually returns capacity");
    }

    #[test]
    fn native_worker_unpolled_capture_drops_before_permit() {
        let permits = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = permits.clone().try_acquire_owned().unwrap();
        let (capture, observed) = observed_capture(&permits);
        let future = run_native_worker(permit, (), move || drop(capture));
        drop(future);
        assert_eq!(observed.load(Ordering::SeqCst), 0);
        assert_eq!(permits.available_permits(), 1);
    }

    #[test]
    fn native_worker_missing_runtime_refuses_and_releases_original_capture() {
        let permits = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = permits.clone().try_acquire_owned().unwrap();
        let (capture, observed) = observed_capture(&permits);
        let mut future = std::pin::pin!(run_native_worker(permit, (), move || drop(capture)));
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(matches!(
            future.as_mut().poll(&mut context),
            std::task::Poll::Ready(Err(NativeWorkerError::RuntimeUnavailable))
        ));
        assert_eq!(observed.load(Ordering::SeqCst), 0);
        assert_eq!(permits.available_permits(), 1);
    }

    #[test]
    fn native_worker_stopped_runtime_discards_unstarted_capture_before_permit() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let handle = runtime.handle().clone();
        drop(runtime);
        let permits = Arc::new(tokio::sync::Semaphore::new(1));
        let (capture, observed) = observed_capture(&permits);
        let (job, receiver, mut ready) = WorkerJob::new(
            permits.clone().try_acquire_owned().unwrap(),
            (),
            move || {
                drop(capture);
                panic!("a stopped runtime must never start the original std worker");
            },
        );
        let joining = handle.spawn_blocking(move || job.run());
        assert!(joining.is_finished());
        assert!(matches!(
            ready.try_recv(),
            Err(oneshot::error::TryRecvError::Closed)
        ));
        assert_eq!(observed.load(Ordering::SeqCst), 0);
        assert_eq!(permits.available_permits(), 1);
        drop(receiver);
    }

    #[test]
    fn native_worker_external_ready_future_drops_before_runtime_shutdown() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        let permits = Arc::new(tokio::sync::Semaphore::new(1));
        let (entered, waiting) = oneshot::channel();
        let (release, released) = std::sync::mpsc::channel();
        let (job, receiver, ready) = WorkerJob::new(
            permits.clone().try_acquire_owned().unwrap(),
            (),
            move || HeldDrop {
                entered: Some(entered),
                release: released,
            },
        );
        let mut future = Box::pin(receiver.receive(ready));
        runtime.block_on(std::future::poll_fn(|context| {
            assert!(future.as_mut().poll(context).is_pending());
            std::task::Poll::Ready(())
        }));
        let (sent, queued) = std::sync::mpsc::channel();
        let joining = runtime.spawn_blocking(move || {
            job.run_with_spawn_and_ready(
                |work| std::thread::Builder::new().spawn(work),
                move || {
                    sent.send(()).unwrap();
                },
            );
        });
        queued.recv().unwrap();
        // The runtime does not own this future. Resolve its result custody before
        // dropping the runtime that owns the original blocking join job.
        drop(future);
        waiting.blocking_recv().unwrap();
        let retained = permits.available_permits() == 0;
        release.send(()).unwrap();
        runtime.block_on(joining).unwrap();
        assert!(retained);
        assert_eq!(permits.available_permits(), 1);
        drop(runtime);
    }

    #[tokio::test]
    async fn native_worker_spawn_refusal_destroys_capture_before_refund() {
        let permits = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = permits.clone().try_acquire_owned().unwrap();
        let (capture, observed) = observed_capture(&permits);
        let (job, receiver, ready) = WorkerJob::new(permit, (), move || drop(capture));
        let joining = tokio::task::spawn_blocking(move || {
            job.run_with_spawn(|work| {
                drop(work);
                Err(std::io::Error::other("injected thread creation refusal"))
            });
        });
        ready.await.unwrap();
        assert_eq!(observed.load(Ordering::SeqCst), 0);
        let completion = receiver.take().unwrap();
        assert!(matches!(
            completion.output,
            Some(Err(NativeWorkerError::ThreadCreation(_)))
        ));
        assert_eq!(permits.available_permits(), 0);
        drop(completion);
        assert_eq!(permits.available_permits(), 1);
        joining.await.unwrap();
    }

    #[test]
    fn native_worker_cancelled_queued_job_keeps_original_admission() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async {
            let (entered, waiting) = oneshot::channel();
            let (release, released) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                entered.send(()).unwrap();
                released.recv().unwrap();
            });
            waiting.await.unwrap();
            let permits = Arc::new(tokio::sync::Semaphore::new(1));
            let permit = permits.clone().try_acquire_owned().unwrap();
            let calls = Arc::new(AtomicUsize::new(0));
            let original_calls = calls.clone();
            let pending = tokio::spawn(run_native_worker(permit, (), move || {
                original_calls.fetch_add(1, Ordering::SeqCst);
            }));
            tokio::task::yield_now().await;
            pending.abort();
            assert!(pending.await.is_err_and(|error| error.is_cancelled()));
            let retained = permits.available_permits() == 0;
            let queued = calls.load(Ordering::SeqCst) == 0;
            release.send(()).unwrap();
            blocker.await.unwrap();
            returned_capacity(&permits).await;
            assert!(
                retained,
                "queued original work remains admitted after cancellation"
            );
            assert!(queued, "std work starts only inside the bounded join job");
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        });
    }

    #[test]
    fn native_worker_waits_for_tls_without_blocking_current_thread_runtime() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async {
            let permits = Arc::new(tokio::sync::Semaphore::new(1));
            let permit = permits.clone().try_acquire_owned().unwrap();
            let (entered, waiting) = tokio::sync::oneshot::channel();
            let (release, released) = std::sync::mpsc::channel();
            let pending = tokio::spawn(run_native_worker(permit, (), move || {
                WORKER_TLS.set(Some(HeldDrop {
                    entered: Some(entered),
                    release: released,
                }));
                37
            }));
            waiting.await.unwrap();
            let retained = permits.available_permits() == 0;
            // Only this executor can release TLS. A synchronous join on it deadlocks.
            release.send(()).unwrap();
            let (value, ()) = pending.await.unwrap().unwrap();
            assert!(retained, "TLS teardown must retain the original permit");
            assert_eq!(value, 37);
            assert_eq!(permits.available_permits(), 1);
        });
    }

    #[tokio::test]
    async fn native_worker_cancelled_custody_survives_tls_exit() {
        let permits = Arc::new(tokio::sync::Semaphore::new(1));
        let (custody, observed) = observed_capture(&permits);
        let (entered, waiting) = oneshot::channel();
        let (release, released) = std::sync::mpsc::channel();
        let pending = tokio::spawn(run_native_worker(
            permits.clone().try_acquire_owned().unwrap(),
            custody,
            move || {
                WORKER_TLS.set(Some(HeldDrop {
                    entered: Some(entered),
                    release: released,
                }));
            },
        ));
        waiting.await.unwrap();
        pending.abort();
        assert!(pending.await.is_err_and(|error| error.is_cancelled()));
        let custody_retained = observed.load(Ordering::SeqCst) == usize::MAX;
        release.send(()).unwrap();
        returned_capacity(&permits).await;
        assert!(
            custody_retained,
            "cancellation cannot release phase custody during TLS teardown"
        );
        assert_eq!(observed.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn native_worker_returns_original_custody_after_refund() {
        let permits = Arc::new(tokio::sync::Semaphore::new(1));
        let custody = Box::new(37_u64);
        let original_address = std::ptr::from_ref(custody.as_ref()) as usize;
        let (value, custody) = run_native_worker(
            permits.clone().try_acquire_owned().unwrap(),
            custody,
            || 19_u64,
        )
        .await
        .unwrap();
        assert_eq!(value, 19);
        assert_eq!(*custody, 37);
        assert_eq!(
            std::ptr::from_ref(custody.as_ref()) as usize,
            original_address
        );
        assert_eq!(permits.available_permits(), 1);
    }

    #[tokio::test]
    async fn native_worker_cancelled_result_drops_before_permit() {
        let permits = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = permits.clone().try_acquire_owned().unwrap();
        let (started, running) = tokio::sync::oneshot::channel();
        let (finish, finishing) = std::sync::mpsc::channel();
        let (entered, waiting) = tokio::sync::oneshot::channel();
        let (release, released) = std::sync::mpsc::channel();
        let pending = tokio::spawn(run_native_worker(permit, (), move || {
            started.send(()).unwrap();
            finishing.recv().unwrap();
            HeldDrop {
                entered: Some(entered),
                release: released,
            }
        }));
        running.await.unwrap();
        pending.abort();
        assert!(pending.await.is_err_and(|error| error.is_cancelled()));
        finish.send(()).unwrap();
        waiting.await.unwrap();
        let retained = permits.available_permits() == 0;
        release.send(()).unwrap();
        returned_capacity(&permits).await;
        assert!(retained, "discarding the original result remains funded");
    }

    #[test]
    fn native_worker_delivered_unclaimed_result_cleanup_leaves_executor() {
        struct DropThread {
            observed: Arc<Mutex<Option<std::thread::ThreadId>>>,
            _held: HeldDrop,
        }
        impl Drop for DropThread {
            fn drop(&mut self) {
                *self.observed.lock().unwrap() = Some(std::thread::current().id());
            }
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async {
            let executor = std::thread::current().id();
            let dropped = Arc::new(Mutex::new(None));
            let original = dropped.clone();
            let permits = Arc::new(tokio::sync::Semaphore::new(1));
            let (entered, waiting) = oneshot::channel();
            let (release, released) = std::sync::mpsc::channel();
            let (job, receiver, ready) = WorkerJob::new(
                permits.clone().try_acquire_owned().unwrap(),
                (),
                move || DropThread {
                    observed: original,
                    _held: HeldDrop {
                        entered: Some(entered),
                        release: released,
                    },
                },
            );
            let (sent, queued) = oneshot::channel();
            let joining = tokio::task::spawn_blocking(move || {
                job.run_with_spawn_and_ready(
                    |work| std::thread::Builder::new().spawn(work),
                    move || {
                        sent.send(()).unwrap();
                    },
                );
            });
            queued.await.unwrap();
            // The actual ready notification is enqueued; its receiver has never
            // been polled, and no result has been claimed.
            drop(ready);
            drop(receiver);
            waiting.await.unwrap();
            let retained = permits.available_permits() == 0;
            // The destructor needs this same executor to release it.
            release.send(()).unwrap();
            joining.await.unwrap();
            assert!(retained);
            assert_ne!(*dropped.lock().unwrap(), Some(executor));
            assert_eq!(permits.available_permits(), 1);
        });
    }

    #[tokio::test]
    async fn native_worker_panic_payload_drops_before_permit() {
        let permits = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = permits.clone().try_acquire_owned().unwrap();
        let (entered, waiting) = tokio::sync::oneshot::channel();
        let (release, released) = std::sync::mpsc::channel();
        let pending = tokio::spawn(run_native_worker(permit, (), move || {
            std::panic::panic_any(HeldDrop {
                entered: Some(entered),
                release: released,
            });
        }));
        waiting.await.unwrap();
        let retained = permits.available_permits() == 0;
        release.send(()).unwrap();
        assert!(matches!(
            pending.await.unwrap(),
            Err(NativeWorkerError::Panicked)
        ));
        returned_capacity(&permits).await;
        assert!(retained, "panic payload destruction remains funded");
    }

    #[tokio::test]
    async fn native_worker_leaves_runtime_and_releases_only_completed_work() {
        let permits = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = permits.clone().try_acquire_owned().unwrap();
        assert!(
            run_native_worker(permit, (), || tokio::runtime::Handle::try_current()
                .is_err())
            .await
            .unwrap()
            .0
        );
        let permit = permits.clone().try_acquire_owned().unwrap();
        let (started, entered) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let pending = tokio::spawn(run_native_worker(permit, (), move || {
            started.send(()).unwrap();
            wait.recv().unwrap();
        }));
        entered.await.unwrap();
        pending.abort();
        assert!(pending.await.unwrap_err().is_cancelled());
        assert!(
            permits.clone().try_acquire_owned().is_err(),
            "dropping the awaiter cannot release its original active worker"
        );
        release.send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if permits.clone().try_acquire_owned().is_ok() {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("completed original worker returns its permit");
    }
}
