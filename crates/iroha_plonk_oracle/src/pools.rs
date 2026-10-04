//! Rayon pools for thread-count independence checks.
//!
//! Every native kernel must return the same bytes at every pool size, and so
//! must the vendored routine it is compared with. The parity tests therefore
//! run each comparison inside dedicated Rayon pools of [`THREAD_COUNTS`]
//! threads. The pools are built once per process and shared by every test.

use std::{fmt::Debug, sync::OnceLock};

use rayon::{ThreadPool, ThreadPoolBuilder};

/// Pool sizes every parity test runs at.
pub const THREAD_COUNTS: [usize; 4] = [1, 2, 4, 7];

/// The shared pools, one per entry of [`THREAD_COUNTS`].
static POOLS: OnceLock<Vec<ThreadPool>> = OnceLock::new();

/// The shared pool with `THREAD_COUNTS[index]` threads.
///
/// Panics when `index` is out of range or a pool cannot be built.
fn shared_pool(index: usize) -> &'static ThreadPool {
    let pools = POOLS.get_or_init(|| {
        THREAD_COUNTS
            .iter()
            .map(|&threads| {
                ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .thread_name(move |worker| format!("oracle-{threads}t-{worker}"))
                    .build()
                    .expect("build a Rayon pool")
            })
            .collect()
    });
    &pools[index]
}

/// Runs `f` once inside each pool of [`THREAD_COUNTS`] threads.
///
/// `f` receives the pool size. Results are returned in [`THREAD_COUNTS`]
/// order.
pub fn on_each_pool<R: Send>(f: impl Fn(usize) -> R + Sync) -> Vec<R> {
    THREAD_COUNTS
        .iter()
        .enumerate()
        .map(|(index, &threads)| shared_pool(index).install(|| f(threads)))
        .collect()
}

/// Runs `f` inside each pool and asserts that every pool returns the same
/// value, which it returns.
///
/// `label` names the computation in the failure message.
pub fn same_on_each_pool<R: PartialEq + Debug + Send>(
    label: &str,
    f: impl Fn(usize) -> R + Sync,
) -> R {
    let mut results = on_each_pool(f).into_iter().zip(THREAD_COUNTS);
    let (first, first_threads) = results.next().expect("at least one pool size");
    for (result, threads) in results {
        assert!(
            result == first,
            "{label}: {threads}-thread result differs from {first_threads}-thread result:\n  \
             {result:?}\n  vs\n  {first:?}"
        );
    }
    first
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_pool_has_its_size() {
        let sizes = on_each_pool(|threads| (threads, rayon::current_num_threads()));
        assert_eq!(sizes.len(), THREAD_COUNTS.len());
        for ((requested, actual), expected) in sizes.into_iter().zip(THREAD_COUNTS) {
            assert_eq!(requested, expected);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn same_on_each_pool_returns_the_shared_value() {
        assert_eq!(same_on_each_pool("constant", |_| 42_u32), 42);
    }

    #[test]
    #[should_panic(expected = "pool-dependent: 2-thread result differs")]
    fn same_on_each_pool_rejects_pool_dependent_values() {
        let _ = same_on_each_pool("pool-dependent", |threads| threads);
    }
}
