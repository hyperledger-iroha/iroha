//! Process-wide admission for blocking `SoraNet` handshake cryptography.

use std::{
    num::NonZeroUsize,
    sync::{Arc, LazyLock, Mutex, Weak},
};

use iroha_config::parameters::actual::SoranetPow as ActualSoranetPow;
use tokio::sync::Semaphore;

use crate::Error;

/// Default number of concurrent outbound puzzle mints.
pub const DEFAULT_OUTBOUND_MINT_CAPACITY: NonZeroUsize =
    ActualSoranetPow::DEFAULT_PUZZLE_WORK_CAPACITY_PER_DIRECTION;
/// Default number of concurrent inbound puzzle verifications.
pub const DEFAULT_INBOUND_VERIFY_CAPACITY: NonZeroUsize =
    ActualSoranetPow::DEFAULT_PUZZLE_WORK_CAPACITY_PER_DIRECTION;

/// Direction-aware process admission for blocking handshake jobs.
#[derive(Debug)]
pub struct SoranetPuzzleWorkAdmission {
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

/// Execute one blocking admission job while retaining its permit even if the
/// surrounding async handshake is cancelled.
pub async fn run_soranet_admission_work<T, F>(gate: Arc<Semaphore>, work: F) -> Result<T, Error>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, Error> + Send + 'static,
{
    let permit = gate.acquire_owned().await.map_err(|error| {
        Error::HandshakeSoranet(format!("SoraNet admission work gate closed: {error}"))
    })?;
    tokio::task::spawn_blocking(move || {
        // Tokio cannot cancel blocking work after a handshake timeout. Holding
        // the permit here prevents the next attempt from overlapping it.
        let _permit = permit;
        work()
    })
    .await
    .map_err(|error| {
        Error::HandshakeSoranet(format!("SoraNet admission work task failed: {error}"))
    })?
}
