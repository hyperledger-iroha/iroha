//! Physical reader ownership issued exclusively by the network connection owner.
//! A cancelled operation drops its I/O and ledger before publishing release;
//! already delivered messages retain their independent, shared `PeerId` owners.
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::sync::{oneshot, watch};

/// Non-cloneable permission for one authenticated connection to bind its reader.
pub struct ReaderPermit {
    _released: oneshot::Sender<()>,
}
impl ReaderPermit {
    /// Create a permit and its exact physical-release observation.
    pub(crate) fn channel() -> (Self, oneshot::Receiver<()>) {
        let (released, receiver) = oneshot::channel();
        (
            Self {
                _released: released,
            },
            receiver,
        )
    }
    /// Own the complete I/O operation before releasing the reader permission.
    /// The one boxed future is bounded by the already admitted connection count.
    pub(crate) fn run<F: Future<Output = ()>>(self, operation: F) -> ReaderOperation<F> {
        ReaderOperation {
            operation: Box::pin(operation),
            _permit: self,
        }
    }
}
/// Field drop order is the cancellation fence: operation first, permit second.
pub struct ReaderOperation<F: Future<Output = ()>> {
    operation: Pin<Box<F>>,
    _permit: ReaderPermit,
}
impl<F: Future<Output = ()>> Future for ReaderOperation<F> {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().operation.as_mut().poll(cx)
    }
}
/// Wait for cancellation even if it was published before this future was polled.
pub async fn cancelled(receiver: &mut watch::Receiver<bool>) {
    loop {
        if *receiver.borrow_and_update() || receiver.changed().await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    struct PhysicalProbe {
        released: Arc<Mutex<oneshot::Receiver<()>>>,
        dropped: Arc<AtomicBool>,
    }
    impl Drop for PhysicalProbe {
        fn drop(&mut self) {
            assert!(
                matches!(
                    self.released.lock().unwrap().try_recv(),
                    Err(oneshot::error::TryRecvError::Empty)
                ),
                "physical I/O must drop before permission"
            );
            self.dropped.store(true, Ordering::SeqCst);
        }
    }
    fn probe() -> (
        ReaderPermit,
        Arc<Mutex<oneshot::Receiver<()>>>,
        Arc<AtomicBool>,
        PhysicalProbe,
    ) {
        let (permit, receiver) = ReaderPermit::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let dropped = Arc::new(AtomicBool::new(false));
        let probe = PhysicalProbe {
            released: Arc::clone(&receiver),
            dropped: Arc::clone(&dropped),
        };
        (permit, receiver, dropped, probe)
    }
    #[test]
    fn cancellation_before_first_poll_drops_owned_io_before_reader_permission() {
        let (permit, receiver, dropped, probe) = probe();
        let operation = permit.run(async move {
            let _physical = probe;
            std::future::pending::<()>().await;
        });
        drop(operation);
        assert!(dropped.load(Ordering::SeqCst));
        assert!(matches!(
            receiver.lock().unwrap().try_recv(),
            Err(oneshot::error::TryRecvError::Closed)
        ));
    }
    #[tokio::test]
    async fn cancellation_after_poll_and_success_both_publish_only_physical_release() {
        for complete in [false, true] {
            let (permit, receiver, dropped, probe) = probe();
            let mut operation = Box::pin(permit.run(async move {
                let _physical = probe;
                if !complete {
                    std::future::pending::<()>().await;
                }
            }));
            if complete {
                operation.await;
            } else {
                assert!(operation.as_mut().now_or_never().is_none());
                assert!(!dropped.load(Ordering::SeqCst));
                drop(operation);
            }
            assert!(dropped.load(Ordering::SeqCst));
            assert!(matches!(
                receiver.lock().unwrap().try_recv(),
                Err(oneshot::error::TryRecvError::Closed)
            ));
        }
    }
    #[tokio::test(start_paused = true)]
    async fn queued_permission_and_partial_geometry_share_original_deadline() {
        use crate::preauth::{DeadlineElapsed, PreauthDeadline};
        use std::time::Duration;
        let deadline = PreauthDeadline::from_now(Duration::from_secs(5)).unwrap();
        let (permit, receiver, dropped, probe) = probe();
        tokio::time::advance(Duration::from_secs(4)).await;
        let result = deadline
            .run(
                None,
                permit.run(async move {
                    let _physical = probe;
                    std::future::pending::<()>().await;
                }),
            )
            .await;
        assert_eq!(result, Err(DeadlineElapsed::Absolute));
        assert!(dropped.load(Ordering::SeqCst));
        assert!(matches!(
            receiver.lock().unwrap().try_recv(),
            Err(oneshot::error::TryRecvError::Closed)
        ));
        let before = tokio::time::Instant::now();
        assert_eq!(
            deadline.run(None, std::future::pending::<()>()).await,
            Err(DeadlineElapsed::Absolute)
        );
        assert_eq!(
            tokio::time::Instant::now(),
            before,
            "no replacement deadline"
        );
    }
    #[tokio::test]
    async fn cancellation_observes_preexisting_signal_and_closed_owner() {
        let (sender, mut receiver) = watch::channel(false);
        sender.send_replace(true);
        assert!(cancelled(&mut receiver).now_or_never().is_some());
        let (sender, mut receiver) = watch::channel(false);
        drop(sender);
        assert!(cancelled(&mut receiver).now_or_never().is_some());
    }
}
