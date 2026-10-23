//! One bounded maintenance worker per opaque wallet, with Core-owned admission.
//!
//! Recovery, receipt verification, one eligible fold sub-proof and one exact Archive
//! attempt share this worker. It parks without custody or proof priority. Coalesced events
//! and relative timer hints select another source read; they never grant admission.

use super::*;
#[cfg(test)]
mod native_maintenance_tests;
mod observation;
mod runner;
pub(crate) use observation::FoldBridgeTurnV1;
use runner::worker;
use std::sync::{Condvar, Weak};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Control {
    enabled: bool,
    stopped: bool,
    started: bool,
    running: bool,
    wake: bool,
    deadline: Option<Instant>,
    pending_attempts: u32,
    payments: usize,
    // Backlog of the last finished turn; `None` when unknown. A hint, never readiness.
    backlog: Option<u128>,
    // A terminal error cannot be overwritten by a later wake before a caller observes it.
    error: Option<(Failure, bool)>,
    // Observation-only fold telemetry of this owner; no scheduling or error path reads it.
    fold: state::FoldMetricsV1,
}
#[derive(Default)]
struct Inner {
    control: Mutex<Control>,
    ready: Condvar,
}
impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, Control> {
        self.control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    fn notify(&self) {
        let mut state = self.lock();
        if !state.stopped {
            state.wake = true;
            self.ready.notify_all();
        }
    }
    fn retry(&self) {
        let mut state = self.lock();
        if state.error.is_some_and(|(_, observed)| observed) {
            state.error = None;
        }
        if !state.stopped {
            state.wake = true;
            self.ready.notify_all();
        }
    }
}
impl state::FoldObserverV1 for Inner {
    fn sub_proof(&self, mut record: state::FoldSubProofV1) {
        record.peak_rss_bytes = peak_rss_bytes();
        let id = {
            let mut control = self.lock();
            control.fold.record(record);
            control.fold.counts().sub_proofs
        };
        observation::capture(self, id);
    }
}
/// Process lifetime peak resident set size, read at the end of each fold sub-proof.
#[cfg(unix)]
fn peak_rss_bytes() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `usage` is writable memory for one `rusage`; RUSAGE_SELF reads this process.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: zero-initialized and then filled by the successful call above.
    let peak = u64::try_from(unsafe { usage.assume_init() }.ru_maxrss).ok()?;
    // Apple platforms report bytes; Linux and Android report kibibytes.
    Some(if cfg!(target_vendor = "apple") {
        peak
    } else {
        peak.saturating_mul(1024)
    })
}
#[cfg(not(unix))]
fn peak_rss_bytes() -> Option<u64> {
    None
}
#[derive(Default)]
enum WorkerJoin {
    #[default]
    NeverStarted,
    Running(JoinHandle<()>),
    Joining,
    Joined,
    Failed(Failure),
}
#[derive(Default)]
pub(super) struct Background {
    inner: Arc<Inner>,
    join: Mutex<WorkerJoin>,
    join_ready: Condvar,
}
impl Background {
    /// A background that keeps the fold telemetry its owner's scheduler reports.
    pub(super) fn observing(scheduler: &state::Scheduler) -> Self {
        let background = Self::default();
        scheduler.observe(background.inner.clone());
        background
    }
    /// Ensure exactly one worker and discover retained work even while inactive.
    pub(super) fn start(&self, owner: &Arc<Owner>) -> Result<()> {
        self.start_with(owner, |task| {
            thread::Builder::new()
                .name("kagemusha-maintenance".into())
                .spawn(task)
        })
    }
    fn start_with(
        &self,
        owner: &Arc<Owner>,
        spawn: impl FnOnce(Box<dyn FnOnce() + Send>) -> std::io::Result<JoinHandle<()>>,
    ) -> Result<()> {
        owner.closing.require_open()?;
        // Serialize spawn with close, but never hold this mutex during a join.
        let mut join = self.join.lock().map_err(|_| Failure::code(INTERNAL))?;
        {
            let mut state = self.inner.lock();
            if state.stopped {
                return Err(Failure::code(CLOSED));
            }
            match &*join {
                WorkerJoin::NeverStarted => {}
                WorkerJoin::Running(_) => return Ok(()),
                WorkerJoin::Joining | WorkerJoin::Joined => return Err(Failure::code(CLOSED)),
                WorkerJoin::Failed(error) => return Err(*error),
            }
            state.started = true;
            state.wake = true;
        }
        let weak = Arc::downgrade(owner);
        let inner = self.inner.clone();
        match spawn(Box::new(move || worker(weak, inner))) {
            Ok(handle) => {
                *join = WorkerJoin::Running(handle);
                Ok(())
            }
            Err(_) => {
                self.inner.lock().started = false;
                Err(Failure::code(RESOURCE))
            }
        }
    }
    /// Publish fold-eligibility telemetry; Core separately owns actual charging admission.
    pub(super) fn activity(&self, owner: &Arc<Owner>, enabled: bool) -> Result<()> {
        {
            let mut control = self.inner.lock();
            if control.stopped {
                return Err(Failure::code(CLOSED));
            }
            control.enabled = enabled;
            control.wake = true;
            self.inner.ready.notify_all();
        }
        if let Err(error) = self.start(owner) {
            self.inner.lock().enabled = false;
            return Err(error);
        }
        Ok(())
    }
    /// Internal/event notifications never acknowledge or clear a sticky failure.
    pub(super) fn notify(&self) {
        self.inner.notify();
    }
    /// A subsequent external request retries once; an unobserved failure remains visible.
    pub(super) fn retry(&self, owner: &Arc<Owner>) -> Result<()> {
        owner.closing.require_open()?;
        self.inner.retry();
        self.start(owner)
    }
    pub(super) fn stop(&self) {
        let mut state = self.inner.lock();
        state.stopped = true;
        state.enabled = false;
        self.inner.ready.notify_all();
    }
    pub(super) fn join(&self) -> Result<()> {
        let mut state = self.join.lock().map_err(|_| Failure::code(INTERNAL))?;
        let handle = loop {
            match &*state {
                WorkerJoin::NeverStarted | WorkerJoin::Joined => {
                    *state = WorkerJoin::Joined;
                    return Ok(());
                }
                WorkerJoin::Failed(error) => return Err(*error),
                WorkerJoin::Joining => {
                    state = self
                        .join_ready
                        .wait(state)
                        .map_err(|_| Failure::code(INTERNAL))?;
                }
                WorkerJoin::Running(_) => {
                    let WorkerJoin::Running(handle) =
                        std::mem::replace(&mut *state, WorkerJoin::Joining)
                    else {
                        unreachable!("running worker owns its join handle");
                    };
                    break handle;
                }
            }
        };
        self.join_ready.notify_all();
        drop(state);
        // Consuming the handle is not a successful join. Keep concurrent callers waiting,
        // and latch any panic before releasing its payload or allowing a close retry.
        let joined = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| handle.join()));
        let result = if matches!(&joined, Ok(Ok(()))) {
            Ok(())
        } else {
            Err(Failure::code(INTERNAL))
        };
        let (mut state, result) = match self.join.lock() {
            Ok(state) => (state, result),
            Err(poisoned) => (poisoned.into_inner(), Err(Failure::code(INTERNAL))),
        };
        *state = match result {
            Ok(()) => WorkerJoin::Joined,
            Err(error) => WorkerJoin::Failed(error),
        };
        self.join_ready.notify_all();
        drop(state);
        drop(joined);
        result
    }
    pub(super) fn payment<'a>(
        &'a self,
        scheduler: &state::Scheduler,
        class: state::FoldCallClassV1,
    ) -> Payment<'a> {
        self.inner.lock().payments += 1;
        self.inner.ready.notify_all();
        Payment {
            priority: Some(scheduler.payment_for(class)),
            background: self,
        }
    }
    /// Keep fold telemetry from the explicit result and notify source discovery after its
    /// fence. Even a caught-up result can release a previously observed Archive-fold wait.
    /// The internal worker does not call this external notification wrapper.
    pub(super) fn after_explicit_fold(&self, result: &Result<Response>) {
        if let Some(Ok((_, backlog))) = result.as_ref().ok().map(finished) {
            if let Some(backlog) = backlog {
                self.inner.lock().backlog = Some(backlog);
            }
        } else {
            self.inner.lock().backlog = None;
        }
        self.inner.notify();
    }
    /// Record a finished execution/speculation observation without scheduling work.
    pub(super) fn record_native_work(&self, record: state::NativeWorkRecordV1) {
        self.inner.lock().fold.record_native_work(record);
    }
    /// Observation-only fold telemetry: the last 64 sub-proofs and cumulative counts.
    pub(super) fn fold_metrics(&self) -> state::FoldMetricsV1 {
        let mut metrics = self.inner.lock().fold.clone();
        metrics.set_process_peak_rss_bytes(peak_rss_bytes());
        metrics
    }
    pub(super) fn status(&self) -> Result<Response> {
        let mut state = self.inner.lock();
        if state.stopped {
            return Err(Failure::code(CLOSED));
        }
        if let Some((error, observed)) = &mut state.error {
            *observed = true;
            return Err(*error);
        }
        Ok(Response {
            kind: 29,
            sequence: state.backlog.unwrap_or(0),
            detail: (if state.running {
                2
            } else {
                u32::from(state.started)
            }) | (u32::from(state.enabled) << 2)
                | (u32::from(state.backlog.is_some()) << 3),
            ..Response::default()
        })
    }
}
pub(super) struct Payment<'a> {
    priority: Option<state::PaymentGuard>,
    background: &'a Background,
}
impl Drop for Payment<'_> {
    fn drop(&mut self) {
        // Release core payment priority before notifying the parked runner.
        drop(self.priority.take());
        self.background.inner.lock().payments -= 1;
        self.background.inner.notify();
    }
}
impl Drop for Background {
    fn drop(&mut self) {
        self.stop();
        // Explicit close joins. Defensive destruction cannot join its own worker when
        // its temporary upgrade happens to be the final owner reference.
        let join = self
            .join
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let WorkerJoin::Running(handle) = std::mem::replace(join, WorkerJoin::Joined)
            && handle.thread().id() != thread::current().id()
        {
            let _ = handle.join();
        }
    }
}
/// Check one fold response and take the backlog of the manifest its turn ended on: none for
/// an idle turn, none or zero when caught up (none before Bootstrap), at least one after a
/// checkpoint and any count after a folded head.
fn finished(response: &Response) -> Result<(i32, Option<u128>)> {
    let shaped = matches!(
        (response.kind, response.backlog),
        (6, None) | (7, None | Some(0)) | (8, Some(1..)) | (9, Some(_))
    );
    if !shaped || !response.bytes.is_empty() {
        return Err(Failure::code(INTERNAL));
    }
    Ok((response.kind, response.backlog))
}

#[cfg(test)]
mod tests;
