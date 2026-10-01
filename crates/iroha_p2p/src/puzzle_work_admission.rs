//! Process-wide admission for blocking `SoraNet` handshake cryptography.

use std::{
    cell::Cell,
    num::NonZeroUsize,
    sync::{
        Arc, LazyLock, Mutex, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use futures::{StreamExt, stream::FuturesUnordered};
#[cfg(test)]
use iroha_config::parameters::actual::SoranetPow as ActualSoranetPow;
use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore, TryAcquireError};

use crate::Error;

/// Default number of concurrent outbound puzzle mints.
#[cfg(test)]
pub const DEFAULT_OUTBOUND_MINT_CAPACITY: NonZeroUsize =
    ActualSoranetPow::DEFAULT_PUZZLE_WORK_CAPACITY_PER_DIRECTION;
/// Default number of concurrent inbound puzzle verifications.
#[cfg(test)]
pub const DEFAULT_INBOUND_VERIFY_CAPACITY: NonZeroUsize =
    ActualSoranetPow::DEFAULT_PUZZLE_WORK_CAPACITY_PER_DIRECTION;

/// Direction-aware process admission for blocking handshake jobs.
#[derive(Debug)]
pub struct SoranetPuzzleWorkAdmission {
    outbound_mint: Arc<Semaphore>,
    inbound_verify: Arc<Semaphore>,
    outbound_mint_capacity: NonZeroUsize,
    inbound_verify_capacity: NonZeroUsize,
    outbound_primary_waiters: AtomicUsize,
    outbound_changed: Notify,
}

impl SoranetPuzzleWorkAdmission {
    pub(crate) fn new(
        outbound_mint_capacity: NonZeroUsize,
        inbound_verify_capacity: NonZeroUsize,
    ) -> Self {
        Self {
            outbound_mint: Arc::new(Semaphore::new(outbound_mint_capacity.get())),
            inbound_verify: Arc::new(Semaphore::new(inbound_verify_capacity.get())),
            outbound_mint_capacity,
            inbound_verify_capacity,
            outbound_primary_waiters: AtomicUsize::new(0),
            outbound_changed: Notify::new(),
        }
    }

    pub(crate) fn capacities(&self) -> (NonZeroUsize, NonZeroUsize) {
        (self.outbound_mint_capacity, self.inbound_verify_capacity)
    }

    #[cfg(test)]
    pub(crate) fn outbound_mint_gate(&self) -> Arc<Semaphore> {
        Arc::clone(&self.outbound_mint)
    }

    pub(crate) fn inbound_verify_gate(&self) -> Arc<Semaphore> {
        Arc::clone(&self.inbound_verify)
    }
}

static PROCESS_WIDE_ADMISSION: LazyLock<Mutex<Weak<SoranetPuzzleWorkAdmission>>> =
    LazyLock::new(|| Mutex::new(Weak::new()));

/// Acquire the one admission authority shared by every production network in
/// this process. Changing its capacities requires a restart so old and new
/// gates can never overlap and exceed the configured memory bound.
pub fn process_wide_admission(
    outbound_mint_capacity: NonZeroUsize,
    inbound_verify_capacity: NonZeroUsize,
) -> Result<Arc<SoranetPuzzleWorkAdmission>, String> {
    let mut slot = PROCESS_WIDE_ADMISSION
        .lock()
        .map_err(|_| "SoraNet puzzle-work admission registry lock poisoned".to_owned())?;
    if let Some(admission) = slot.upgrade() {
        if admission.capacities() == (outbound_mint_capacity, inbound_verify_capacity) {
            return Ok(admission);
        }
        return Err(format!(
            "SoraNet puzzle-work capacities cannot change while the network runtime is active; restart required (active outbound_mint={}, inbound_verify={}; requested outbound_mint={}, inbound_verify={})",
            admission.outbound_mint_capacity,
            admission.inbound_verify_capacity,
            outbound_mint_capacity,
            inbound_verify_capacity,
        ));
    }
    let admission = Arc::new(SoranetPuzzleWorkAdmission::new(
        outbound_mint_capacity,
        inbound_verify_capacity,
    ));
    *slot = Arc::downgrade(&admission);
    Ok(admission)
}

/// Cooperative lifetime shared with one blocking admission job.
pub struct SoranetAdmissionCancellation(Arc<AtomicBool>);

impl SoranetAdmissionCancellation {
    /// Whether the asynchronous owner has stopped waiting for this work.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

struct CancelAdmissionOnDrop(Arc<AtomicBool>);

impl Drop for CancelAdmissionOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// A search either produces a credential or yields spare capacity to a primary.
pub(crate) enum SoranetOutboundWorkOutcome<T> {
    /// A complete credential produced under the unchanged puzzle policy.
    Completed(T),
    /// A spare search stopped at a checkpoint to admit another primary.
    Yielded,
}

/// Cooperative cancellation and primary priority at cryptographic checkpoints.
pub(crate) struct SoranetOutboundWorkControl {
    cancellation: SoranetAdmissionCancellation,
    admission: Arc<SoranetPuzzleWorkAdmission>,
    speculative: bool,
    yielded: Cell<bool>,
}

impl SoranetOutboundWorkControl {
    /// Check before and after each indivisible cryptographic evaluation.
    pub(crate) fn should_continue(&self) -> bool {
        if self.cancellation.is_cancelled() {
            return false;
        }
        if self.speculative
            && self
                .admission
                .outbound_primary_waiters
                .load(Ordering::Acquire)
                != 0
        {
            self.yielded.set(true);
        }
        !self.yielded.get()
    }

    /// Whether a cancellation checkpoint yielded this helper to a primary.
    pub(crate) fn yielded(&self) -> bool {
        self.yielded.get()
    }
}

struct OutboundPrimaryWaiter(Arc<SoranetPuzzleWorkAdmission>);

impl OutboundPrimaryWaiter {
    fn new(admission: Arc<SoranetPuzzleWorkAdmission>) -> Self {
        admission
            .outbound_primary_waiters
            .fetch_add(1, Ordering::AcqRel);
        Self(admission)
    }
}

impl Drop for OutboundPrimaryWaiter {
    fn drop(&mut self) {
        self.0
            .outbound_primary_waiters
            .fetch_sub(1, Ordering::AcqRel);
        self.0.outbound_changed.notify_waiters();
    }
}

struct OutboundWorkPermit {
    permit: Option<OwnedSemaphorePermit>,
    // Retain the registry owner as well as its semaphore: runtime teardown
    // cannot create a second gate while detached blocking work is still live.
    admission: Arc<SoranetPuzzleWorkAdmission>,
}

impl Drop for OutboundWorkPermit {
    fn drop(&mut self) {
        drop(self.permit.take());
        self.admission.outbound_changed.notify_waiters();
    }
}

async fn run_admitted_outbound_work<T, W>(
    permit: OutboundWorkPermit,
    speculative: bool,
    work: W,
) -> Result<SoranetOutboundWorkOutcome<T>, Error>
where
    T: Send + 'static,
    W: FnOnce(SoranetOutboundWorkControl) -> Result<SoranetOutboundWorkOutcome<T>, Error>
        + Send
        + 'static,
{
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel_on_drop = CancelAdmissionOnDrop(Arc::clone(&cancelled));
    tokio::task::spawn_blocking(move || {
        let control = SoranetOutboundWorkControl {
            cancellation: SoranetAdmissionCancellation(cancelled),
            admission: Arc::clone(&permit.admission),
            speculative,
            yielded: Cell::new(false),
        };
        let _permit = permit;
        if !control.should_continue() {
            return if control.yielded() {
                Ok(SoranetOutboundWorkOutcome::Yielded)
            } else {
                Err(Error::HandshakeSoranet(
                    "SoraNet admission work was cancelled before execution".to_owned(),
                ))
            };
        }
        work(control)
    })
    .await
    .map_err(|error| {
        Error::HandshakeSoranet(format!("SoraNet admission work task failed: {error}"))
    })?
}

/// Search with one fair primary and helpers using only idle outbound permits.
///
/// Helpers yield at the next cryptographic checkpoint when another handshake
/// queues its primary. Every blocking job retains the original permit and
/// admission owner until it actually exits, including after a winner or timeout.
/// `make_work` must create an independent search, never clone an RNG stream.
pub(crate) async fn run_soranet_outbound_search<T, F, W>(
    admission: Arc<SoranetPuzzleWorkAdmission>,
    mut make_work: F,
) -> Result<T, Error>
where
    T: Send + 'static,
    F: FnMut() -> W + Send,
    W: FnOnce(SoranetOutboundWorkControl) -> Result<SoranetOutboundWorkOutcome<T>, Error>
        + Send
        + 'static,
{
    let waiter = OutboundPrimaryWaiter::new(Arc::clone(&admission));
    let permit = Arc::clone(&admission.outbound_mint)
        .acquire_owned()
        .await
        .map_err(|error| {
            Error::HandshakeSoranet(format!("SoraNet admission work gate closed: {error}"))
        })?;
    let primary = OutboundWorkPermit {
        permit: Some(permit),
        admission: Arc::clone(&admission),
    };
    drop(waiter);
    let mut searches = FuturesUnordered::new();
    searches.push(run_admitted_outbound_work(primary, false, make_work()));

    loop {
        // Register before inspecting capacity so releases cannot be missed.
        let changed = admission.outbound_changed.notified();
        tokio::pin!(changed);
        changed.as_mut().enable();
        while searches.len() < admission.outbound_mint_capacity.get()
            && admission.outbound_primary_waiters.load(Ordering::Acquire) == 0
        {
            let permit = match Arc::clone(&admission.outbound_mint).try_acquire_owned() {
                Ok(permit) => permit,
                Err(TryAcquireError::NoPermits) => break,
                Err(error @ TryAcquireError::Closed) => {
                    return Err(Error::HandshakeSoranet(format!(
                        "SoraNet admission work gate closed: {error}"
                    )));
                }
            };
            let helper = OutboundWorkPermit {
                permit: Some(permit),
                admission: Arc::clone(&admission),
            };
            // A primary can register between the check and try_acquire.
            if admission.outbound_primary_waiters.load(Ordering::Acquire) != 0 {
                drop(helper);
                break;
            }
            searches.push(run_admitted_outbound_work(helper, true, make_work()));
        }
        tokio::select! {
            biased;
            result = searches.next() => match result {
                Some(Ok(SoranetOutboundWorkOutcome::Completed(value))) => return Ok(value),
                Some(Ok(SoranetOutboundWorkOutcome::Yielded)) => {},
                Some(Err(error)) => return Err(error),
                None => return Err(Error::HandshakeSoranet(
                    "SoraNet admission primary search ended without a credential".to_owned(),
                )),
            },
            () = &mut changed => {},
        }
    }
}

/// Execute one blocking admission job while retaining its permit until the
/// current cryptographic evaluation exits. Cancellation stops subsequent work.
pub async fn run_soranet_admission_work<T, F>(gate: Arc<Semaphore>, work: F) -> Result<T, Error>
where
    T: Send + 'static,
    F: FnOnce(SoranetAdmissionCancellation) -> Result<T, Error> + Send + 'static,
{
    let permit = gate.acquire_owned().await.map_err(|error| {
        Error::HandshakeSoranet(format!("SoraNet admission work gate closed: {error}"))
    })?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let _cancel_on_drop = CancelAdmissionOnDrop(Arc::clone(&cancelled));
    tokio::task::spawn_blocking(move || {
        // Tokio cannot interrupt an Argon2 evaluation. Keep its memory charged
        // while the cooperative token prevents another evaluation after expiry.
        let _permit = permit;
        let cancellation = SoranetAdmissionCancellation(cancelled);
        if cancellation.is_cancelled() {
            return Err(Error::HandshakeSoranet(
                "SoraNet admission work was cancelled before execution".to_owned(),
            ));
        }
        work(cancellation)
    })
    .await
    .map_err(|error| {
        Error::HandshakeSoranet(format!("SoraNet admission work task failed: {error}"))
    })?
}

#[cfg(test)]
mod tests;
