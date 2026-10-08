//! One retained proof thread. Supervised shutdown waits for the exact worker join.
//! Startup rollback cancels and transfers the join handle to a native helper; Drop
//! may return first, but the producer retains cache/journal locks until it exits.
use super::{Job, Queue, Result, Status};
use iroha_core_zk::kagemusha_wallet_finality_v1::server::ServerFinalityCancellationV1;
use iroha_futures::supervisor::ShutdownSignal;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

pub(super) struct Worker {
    sender: Mutex<Option<mpsc::SyncSender<Job>>>,
    queue: Arc<Mutex<Queue>>,
    stop: Arc<AtomicBool>,
    cancel: ServerFinalityCancellationV1,
    thread: Mutex<Option<thread::JoinHandle<bool>>>,
    completed: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
}
impl Worker {
    #[cfg(test)]
    pub(super) fn start(
        capacity: usize,
        cancel: ServerFinalityCancellationV1,
        produce: impl FnMut(&Job, &AtomicBool) -> Result<Vec<u8>> + Send + 'static,
    ) -> std::io::Result<Self> {
        Self::start_with(capacity, cancel, move || produce)
    }

    /// Construct, use and destroy potentially non-Send proving state on its one owner thread.
    pub(super) fn start_with<P>(
        capacity: usize,
        cancel: ServerFinalityCancellationV1,
        create: impl FnOnce() -> P + Send + 'static,
    ) -> std::io::Result<Self>
    where
        P: FnMut(&Job, &AtomicBool) -> Result<Vec<u8>> + 'static,
    {
        if !(1..=64).contains(&capacity) {
            return Err(std::io::Error::other("invalid finality queue bound"));
        }
        let queue = Arc::new(Mutex::new(Queue {
            entries: BTreeMap::new(),
            maximum: capacity,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let (sender, commands) = mpsc::sync_channel::<Job>(capacity);
        let (finished, completed) = tokio::sync::oneshot::channel();
        let worker_queue = Arc::clone(&queue);
        let worker_stop = Arc::clone(&stop);
        let thread = thread::Builder::new()
            .name("kagemusha-load-finality".into())
            .spawn(move || {
                let mut produce = create();
                while let Ok(job) = commands.recv() {
                    if worker_stop.load(Ordering::Acquire) {
                        break;
                    }
                    let result = produce(&job, &worker_stop);
                    let Ok(mut queue) = worker_queue.lock() else {
                        return false;
                    };
                    if worker_stop.load(Ordering::Acquire) {
                        break;
                    }
                    let status = match result {
                        Ok(bytes) => Status::Ready(bytes),
                        Err(reason) => {
                            iroha_logger::warn!(
                                reason,
                                "KAGEMUSHA terminal Load proof unavailable"
                            );
                            Status::Failed
                        }
                    };
                    if let Some(entry) = queue.entries.get_mut(&job.key) {
                        *entry = status;
                    }
                }
                // Completion follows actual producer/journal destruction, including queued DATA.
                // A panic closes finished and is distinguished by the exact thread join result.
                drop(commands);
                drop(produce);
                let _ = finished.send(());
                true
            })?;
        Ok(Self {
            sender: Mutex::new(Some(sender)),
            queue,
            stop,
            cancel,
            thread: Mutex::new(Some(thread)),
            completed: Mutex::new(Some(completed)),
        })
    }
    pub(super) fn read_or_schedule(&self, job: Job) -> Result<Option<Vec<u8>>> {
        // Admission and shutdown share this lock; no request can enqueue after close.
        let sender = self
            .sender
            .lock()
            .map_err(|_| "worker admission unavailable")?;
        let sender = sender.as_ref().ok_or("worker stopped")?;
        let mut queue = self.queue.lock().map_err(|_| "worker queue unavailable")?;
        if let Some(result) = queue.observe(job.key)? {
            return Ok(result);
        }
        let key = job.key;
        if sender.try_send(job).is_err() {
            queue.entries.remove(&key);
            return Err("worker unavailable");
        }
        Ok(None)
    }
    pub(super) fn supervise(
        self: &Arc<Self>,
        shutdown: ShutdownSignal,
    ) -> std::result::Result<tokio::task::JoinHandle<crate::ToriiCriticalWorkerExit>, &'static str>
    {
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| "finality supervision requires the existing async runtime")?;
        let completed = self
            .completed
            .lock()
            .map_err(|_| "finality completion state is poisoned")?
            .take()
            .ok_or("finality worker was already registered")?;
        let owner = Arc::clone(self);
        Ok(runtime.spawn(async move {
            let (requested, completed_normally) = tokio::select! {
                () = shutdown.receive() => (true, true),
                result = completed => (shutdown.is_sent(), result.is_ok()),
            };
            owner.close_admission();
            let joined = tokio::task::spawn_blocking(move || owner.close_and_join()).await;
            if requested && completed_normally && matches!(joined, Ok(true)) {
                crate::ToriiCriticalWorkerExit::StoppedByShutdown
            } else {
                crate::ToriiCriticalWorkerExit::UnexpectedExit
            }
        }))
    }
    fn close_admission(&self) {
        let mut sender = self
            .sender
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Publication shares this boundary. Previously retained journal originals remain valid.
        let _queue = self
            .queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.stop.store(true, Ordering::Release);
        self.cancel.cancel();
        drop(sender.take());
    }
    fn close_and_join(&self) -> bool {
        self.close_admission();
        self.thread
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .is_some_and(|thread| matches!(thread.join(), Ok(true)))
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.close_admission();
        if let Some(thread) = self
            .thread
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            // Startup rollback can run inside Tokio before supervision. This Drop does not
            // normally wait: a native helper retains and joins the exact worker handle.
            // The producer still owns its cache/journal locks until actual destruction,
            // so a subsequent owner cannot acquire them early. Helper-creation failure
            // falls back to joining synchronously; no producer handle is discarded.
            let retained = Arc::new(Mutex::new(Some(thread)));
            let joiner = Arc::clone(&retained);
            if thread::Builder::new()
                .name("kagemusha-finality-join".into())
                .spawn(move || {
                    if let Some(thread) = joiner
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .take()
                    {
                        let _ = thread.join();
                    }
                })
                .is_err()
            {
                if let Some(thread) = retained
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take()
                {
                    let _ = thread.join();
                }
            }
        }
    }
}
#[cfg(test)]
mod tests;
