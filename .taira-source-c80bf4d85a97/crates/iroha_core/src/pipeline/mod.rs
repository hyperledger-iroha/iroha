//! Pipeline utilities: access-set derivation and future scheduler glue.
pub mod access;
pub mod gpu;
pub mod overlay;
pub mod smallset;
/// Background ZK verification lane (non-forking).
pub mod zk_lane;

use std::sync::atomic::{AtomicBool, Ordering};

/// Global switch forcing the scheduler to respect submission/FIFO order.
///
/// This is primarily used by integration tests (e.g., sandbox harness) which
/// need deterministic transaction ordering even when the production scheduler
/// would normally reorder conflict-free overlays.
static FORCE_FIFO_SCHEDULER: AtomicBool = AtomicBool::new(false);

/// Enable or disable the FIFO scheduler override, returning the previous value.
pub fn set_force_fifo_scheduler(enabled: bool) -> bool {
    FORCE_FIFO_SCHEDULER.swap(enabled, Ordering::Relaxed)
}

/// Returns `true` when the FIFO scheduler override is active.
pub fn force_fifo_scheduler() -> bool {
    FORCE_FIFO_SCHEDULER.load(Ordering::Relaxed)
}
