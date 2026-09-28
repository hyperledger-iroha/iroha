//! Host-facing concurrency settings and state-access declarations for IVM.
//!
//! Contract execution is sequential. This module records the declared state
//! access sets hosts use for conflict tracking, the buffered state updates they
//! publish, and the operator-configured scheduler thread limits and worker stack
//! size applied to host thread pools.
use std::{
    collections::HashSet,
    sync::atomic::{AtomicUsize, Ordering},
};
/// Number of general purpose registers in the VM register file.
pub const REGISTER_COUNT: usize = 256;
/// Identifier for a state entry in the world state. For the purposes of this crate it is simply a
/// string key but in a real integration this could be a complex type.
pub type StateKey = String;
/// Generic state value type.
pub type Value = u64;
/// Update produced by a transaction execution.
#[derive(Clone, Debug)]
pub struct StateUpdate {
    pub key: StateKey,
    pub value: Value,
}
/// Stack size allocated for host Rayon worker threads (see [`crate::init_global_rayon`]).
///
/// This value is mutable via [`set_thread_stack_size`] to allow hosts to align
/// worker stacks with deployment policy.
static THREAD_STACK_SIZE: AtomicUsize = AtomicUsize::new(32 * 1024 * 1024);
/// Current Rayon worker stack size used by host pools.
pub fn thread_stack_size() -> usize {
    THREAD_STACK_SIZE.load(Ordering::Relaxed)
}
/// Override the Rayon worker stack size used by host pools.
pub fn set_thread_stack_size(bytes: usize) {
    THREAD_STACK_SIZE.store(bytes.max(1), Ordering::Relaxed);
}
// Global defaults for scheduler thread limits (set by the host/node).
// 0 means "auto" (use physical cores).
static DEFAULT_SCHED_MIN: AtomicUsize = AtomicUsize::new(0);
static DEFAULT_SCHED_MAX: AtomicUsize = AtomicUsize::new(0);
/// Set global default scheduler thread limits reported by [`crate::IVM`] construction.
///
/// - Pass `None` to keep "auto" for that bound (uses physical cores).
/// - Bounds are clamped to at least 1 and `min <= max` is enforced by clamping `min` down.
pub fn set_default_scheduler_limits(min_threads: Option<usize>, max_threads: Option<usize>) {
    let min = min_threads.unwrap_or(0);
    let max = max_threads.unwrap_or(0);
    // Store raw values (0 = auto) and validate when read.
    DEFAULT_SCHED_MIN.store(min, Ordering::SeqCst);
    DEFAULT_SCHED_MAX.store(max, Ordering::SeqCst);
}
/// Read global default scheduler limits as concrete `(min, max)` counts.
///
/// 0 values are resolved to the current number of physical cores.
pub fn default_scheduler_limits() -> (usize, usize) {
    let phys = num_cpus::get_physical().max(1);
    let mut min = DEFAULT_SCHED_MIN.load(Ordering::SeqCst);
    let mut max = DEFAULT_SCHED_MAX.load(Ordering::SeqCst);
    if min == 0 {
        min = phys;
    }
    if max == 0 {
        max = phys;
    }
    min = min.max(1);
    max = max.max(1);
    if max < min {
        // Respect the configured max bound; clamp the minimum down.
        min = max;
    }
    (min, max)
}
/// Read and write sets associated with a transaction for conflict detection.
#[derive(Clone, Debug)]
pub struct StateAccessSet {
    pub read_keys: HashSet<StateKey>,
    pub write_keys: HashSet<StateKey>,
    /// Optional register tags used for additional conflict detection.
    pub reg_tags: HashSet<usize>,
}
impl StateAccessSet {
    pub fn new() -> Self {
        Self {
            read_keys: HashSet::new(),
            write_keys: HashSet::new(),
            reg_tags: HashSet::new(),
        }
    }
}
impl Default for StateAccessSet {
    fn default() -> Self {
        Self::new()
    }
}
