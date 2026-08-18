//! Process-wide admission for memory-hard SoraNet handshake work.

use std::{
    num::NonZeroUsize,
    sync::{Arc, LazyLock, Mutex, Weak},
};

use tokio::sync::Semaphore;

use crate::Error;

/// Default number of concurrent outbound puzzle mints.
pub(crate) const DEFAULT_OUTBOUND_MINT_CAPACITY: NonZeroUsize =
    NonZeroUsize::new(1).expect("one is non-zero");
/// Default number of concurrent inbound puzzle verifications.
pub(crate) const DEFAULT_INBOUND_VERIFY_CAPACITY: NonZeroUsize =
    NonZeroUsize::new(1).expect("one is non-zero");

/// Direction-aware process admission for Argon2 handshake jobs.
#[derive(Debug)]
pub(crate) struct SoranetPuzzleWorkAdmission {
    outbound_mint: Arc<Semaphore>,
    inbound_verify: Arc<Semaphore>,
    outbound_mint_capacity: NonZeroUsize,
    inbound_verify_capacity: NonZeroUsize,
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
        }
    }

    pub(crate) fn capacities(&self) -> (NonZeroUsize, NonZeroUsize) {
        (self.outbound_mint_capacity, self.inbound_verify_capacity)
    }

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
pub(crate) fn process_wide_admission(
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

/// Execute one blocking Argon2 job while retaining its permit even if the
/// surrounding async handshake is cancelled.
pub(crate) async fn run_soranet_puzzle_work<T, F>(gate: Arc<Semaphore>, work: F) -> Result<T, Error>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, Error> + Send + 'static,
{
    let permit = gate.acquire_owned().await.map_err(|error| {
        Error::HandshakeSoranet(format!("SoraNet puzzle work gate closed: {error}"))
    })?;
    tokio::task::spawn_blocking(move || {
        // Tokio cannot cancel blocking work after a handshake timeout. Holding
        // the permit here prevents the next attempt from overlapping it.
        let _permit = permit;
        work()
    })
    .await
    .map_err(|error| Error::HandshakeSoranet(format!("SoraNet puzzle work task failed: {error}")))?
}
