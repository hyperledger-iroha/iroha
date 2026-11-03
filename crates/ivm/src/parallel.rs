//! Host-facing concurrency settings and state-access declarations for IVM.
//!
//! Contract execution is sequential. This module records the declared state
//! access sets hosts use for conflict tracking, the buffered state updates they
//! publish, and the operator-configured scheduler thread limits and worker stack
//! size applied to host thread pools.
use std::{
    collections::HashSet,
    io,
    num::NonZeroUsize,
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
// 0 means "auto" (use the process's available parallelism).
static DEFAULT_SCHED_MIN: AtomicUsize = AtomicUsize::new(0);
static DEFAULT_SCHED_MAX: AtomicUsize = AtomicUsize::new(0);
/// Set global default scheduler thread limits reported by [`crate::IVM`] construction.
///
/// - Pass `None` to keep "auto" for that bound (uses available parallelism).
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
/// Zero bounds use the standard library's estimate of available parallelism,
/// falling back to one worker if the operating system cannot provide it.
/// Explicit bounds avoid querying the operating system.
pub fn default_scheduler_limits() -> (usize, usize) {
    resolve_scheduler_limits(
        DEFAULT_SCHED_MIN.load(Ordering::SeqCst),
        DEFAULT_SCHED_MAX.load(Ordering::SeqCst),
        std::thread::available_parallelism,
    )
}

/// Resolve zero to the process's available parallelism, or one on query failure.
pub(crate) fn configured_thread_count(configured: usize) -> usize {
    resolve_thread_count(configured, std::thread::available_parallelism)
}

fn resolve_thread_count(
    configured: usize,
    query: impl FnOnce() -> io::Result<NonZeroUsize>,
) -> usize {
    if configured == 0 {
        query().map_or(1, NonZeroUsize::get)
    } else {
        configured
    }
}

fn resolve_scheduler_limits(
    mut min: usize,
    mut max: usize,
    query: impl FnOnce() -> io::Result<NonZeroUsize>,
) -> (usize, usize) {
    // Query at most once and only when a bound actually needs an automatic value.
    let available = if min == 0 || max == 0 {
        resolve_thread_count(0, query)
    } else {
        1
    };
    if min == 0 {
        min = available;
    }
    if max == 0 {
        max = available;
    }
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

#[cfg(test)]
mod thread_count_tests {
    use super::*;

    #[test]
    fn explicit_thread_count_does_not_query_the_operating_system() {
        for count in [1, 4, usize::MAX] {
            assert_eq!(
                resolve_thread_count(count, || panic!("explicit count must not query")),
                count,
            );
            assert_eq!(configured_thread_count(count), count);
        }
    }

    #[test]
    fn automatic_thread_count_uses_available_parallelism_or_one() {
        assert_eq!(
            resolve_thread_count(0, || Ok(NonZeroUsize::new(7).unwrap())),
            7
        );
        assert_eq!(
            resolve_thread_count(0, || Err(io::Error::other("unavailable"))),
            1
        );
        assert!(configured_thread_count(0) >= 1);
    }

    #[test]
    fn explicit_scheduler_limits_skip_query_and_preserve_the_maximum() {
        for (min, max, expected) in [(2, 8, (2, 8)), (8, 2, (2, 2)), (1, 1, (1, 1))] {
            assert_eq!(
                resolve_scheduler_limits(min, max, || panic!("explicit bounds must not query")),
                expected,
            );
        }
    }

    #[test]
    fn automatic_scheduler_limits_query_once_and_clamp_the_minimum() {
        for (min, max, expected) in [
            (0, 0, (6, 6)),
            (0, 4, (4, 4)),
            (0, 8, (6, 8)),
            (3, 0, (3, 6)),
            (8, 0, (6, 6)),
        ] {
            let mut calls = 0;
            assert_eq!(
                resolve_scheduler_limits(min, max, || {
                    calls += 1;
                    Ok(NonZeroUsize::new(6).unwrap())
                }),
                expected,
            );
            assert_eq!(calls, 1);
        }
    }

    #[test]
    fn unavailable_parallelism_keeps_scheduler_limits_positive_and_ordered() {
        for (min, max, expected) in [(0, 0, (1, 1)), (0, 4, (1, 4)), (4, 0, (1, 1))] {
            assert_eq!(
                resolve_scheduler_limits(min, max, || Err(io::Error::other("unavailable"))),
                expected,
            );
        }
    }
}
