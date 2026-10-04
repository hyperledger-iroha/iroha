//! Bounded physical seed concurrency; every virtual World stays on its creating thread.

use std::{
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

/// Also bound concurrent sweeps in one libtest process, independently of its test threads.
static BATCH_OWNER: Mutex<()> = Mutex::new(());
const WORKERS: usize = 4;
const WINDOW: usize = WORKERS * 4;

/// Run complete independent seeds and fold their owned observations in input order.
/// No World or host crosses a thread boundary; a worker must release it before returning.
pub(super) fn fold_seeds<T: Send>(
    seeds: impl IntoIterator<Item = u64>,
    work: impl Fn(u64) -> T + Sync,
    fold: impl FnMut(u64, T),
) {
    fold_seed_batches(seeds, WORKERS, work, fold);
}

pub(super) fn fold_seed_batches<T: Send>(
    seeds: impl IntoIterator<Item = u64>,
    workers: usize,
    work: impl Fn(u64) -> T + Sync,
    mut fold: impl FnMut(u64, T),
) {
    assert!((1..=WORKERS).contains(&workers));
    let mut seeds = seeds.into_iter();
    loop {
        let batch: Vec<_> = seeds.by_ref().take(WINDOW).collect();
        if batch.is_empty() {
            break;
        }
        // Release the mutex before propagating a worker panic, so later tests can still run.
        let owner = BATCH_OWNER.lock().expect("seed batch owner");
        let worker_results = if workers == 1 {
            vec![Ok(batch
                .iter()
                .enumerate()
                .map(|(index, &seed)| (index, catch_unwind(AssertUnwindSafe(|| work(seed)))))
                .collect::<Vec<_>>())]
        } else {
            let next = AtomicUsize::new(0);
            std::thread::scope(|scope| {
                let work = &work;
                let next = &next;
                let batch = &batch;
                let mut handles = Vec::with_capacity(workers);
                for _ in 0..workers {
                    handles.push(scope.spawn(move || {
                        let mut completed = Vec::new();
                        loop {
                            let index = next.fetch_add(1, Ordering::Relaxed);
                            let Some(&seed) = batch.get(index) else {
                                break;
                            };
                            // Unwind a failed World completely, then continue independent seeds.
                            let result = catch_unwind(AssertUnwindSafe(|| work(seed)));
                            completed.push((index, result));
                        }
                        completed
                    }));
                }
                // Every handle is joined before any returned result is inspected.
                handles
                    .into_iter()
                    .map(std::thread::ScopedJoinHandle::join)
                    .collect::<Vec<_>>()
            })
        };
        drop(owner);
        let mut results = std::iter::repeat_with(|| None)
            .take(batch.len())
            .collect::<Vec<_>>();
        let mut unexpected_panic = None;
        for joined in worker_results {
            match joined {
                Ok(completed) => {
                    for (index, result) in completed {
                        assert!(results[index].replace(result).is_none(), "seed ran twice");
                    }
                }
                Err(panic) => {
                    unexpected_panic.get_or_insert(panic);
                }
            }
        }
        if let Some(panic) = unexpected_panic {
            resume_unwind(panic);
        }
        for (seed, result) in batch.into_iter().zip(results) {
            match result.expect("every seed returned one observation or panic") {
                Ok(observation) => fold(seed, observation),
                Err(panic) => resume_unwind(panic),
            }
        }
    }
}

/// Keep only the first ordered failure report and a count, even for a large failing campaign.
#[derive(Default)]
pub(super) struct Failures {
    pub(super) passed: usize,
    failed: usize,
    first: Option<(u64, String)>,
}

impl Failures {
    pub(super) fn observe<T>(&mut self, seed: u64, result: Result<T, String>) -> Option<T> {
        match result {
            Ok(value) => {
                self.passed += 1;
                Some(value)
            }
            Err(report) => {
                self.failed += 1;
                self.first.get_or_insert((seed, report));
                None
            }
        }
    }

    pub(super) fn failed(&self) -> usize {
        self.failed
    }

    pub(super) fn finish(self, name: &str) {
        if let Some((seed, report)) = self.first {
            panic!(
                "{name}: {} failing seeds; first (seed {seed}):\n{report}",
                self.failed
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        message::BlockHeader,
        sim::{run, scenarios, world::World},
        types::{Hash32, Millis},
    };
    use norito::codec::Encode;
    use std::sync::{
        Arc, Condvar,
        atomic::{AtomicUsize, Ordering},
    };

    struct Live(Arc<AtomicUsize>);
    impl Drop for Live {
        fn drop(&mut self) {
            self.0.fetch_sub(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn batches_release_workers_and_bound_ordered_observations() {
        let live = Arc::new(AtomicUsize::new(0));
        let peak = AtomicUsize::new(0);
        let mut ordered = Vec::new();
        let pulled = AtomicUsize::new(0);
        fold_seeds(
            (0..37).inspect(|_| {
                pulled.fetch_add(1, Ordering::SeqCst);
            }),
            |seed| {
                let count = live.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(count, Ordering::SeqCst);
                let owner = Live(Arc::clone(&live));
                let observation = seed * 2;
                drop(owner);
                observation
            },
            |seed, value| {
                assert_eq!(
                    live.load(Ordering::SeqCst),
                    0,
                    "all sibling owners released"
                );
                assert!(
                    pulled.load(Ordering::SeqCst) - ordered.len() <= WINDOW,
                    "too many pending observations"
                );
                ordered.push((seed, value));
            },
        );
        assert!((1..=WORKERS).contains(&peak.load(Ordering::SeqCst)));
        assert_eq!(
            ordered,
            (0..37).map(|seed| (seed, seed * 2)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn panics_choose_seed_order_after_releasing_every_sibling() {
        let later_finished = (Mutex::new(false), Condvar::new());
        let live = Arc::new(AtomicUsize::new(0));
        let completed = AtomicUsize::new(0);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fold_seeds(
                0..16,
                |seed| {
                    live.fetch_add(1, Ordering::SeqCst);
                    let _owner = Live(Arc::clone(&live));
                    if seed == 0 {
                        let (lock, wake) = &later_finished;
                        let ready = lock.lock().unwrap();
                        drop(wake.wait_while(ready, |ready| !*ready).unwrap());
                        panic!("earlier seed");
                    }
                    if seed == 1 {
                        *later_finished.0.lock().unwrap() = true;
                        later_finished.1.notify_one();
                        panic!("later seed");
                    }
                    completed.fetch_add(1, Ordering::SeqCst);
                },
                |_, ()| panic!("no observation precedes the first failure"),
            );
        }));
        let panic = result.expect_err("both workers fail");
        assert_eq!(panic.downcast_ref::<&str>(), Some(&"earlier seed"));
        assert_eq!(live.load(Ordering::SeqCst), 0);
        assert_eq!(completed.load(Ordering::SeqCst), 14);
        fold_seeds([4], |seed| seed, |seed, value| assert_eq!(seed, value));
    }

    #[test]
    fn ordinary_failures_keep_first_seed_and_count_without_retaining_reports() {
        let mut failures = Failures::default();
        fold_seeds(
            5..14,
            |seed| Err::<(), _>(format!("failure {seed}")),
            |seed, result| {
                failures.observe(seed, result);
            },
        );
        assert_eq!(failures.failed(), 9);
        assert_eq!(failures.passed, 0);
        assert_eq!(
            failures.first.as_ref().unwrap(),
            &(5, "failure 5".to_owned())
        );
    }

    #[derive(Debug, PartialEq, Eq)]
    struct Reference {
        height: u64,
        hash: Hash32,
        result: Hash32,
        header: BlockHeader,
        at: Millis,
        honest: bool,
        view: u64,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct Certificate {
        header: Vec<u8>,
        qc: Vec<u8>,
        availability: Vec<u8>,
        payload: Vec<u8>,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct CompleteObservation {
        references: Vec<Vec<Reference>>,
        certificates: Vec<Vec<Certificate>>,
        counters: [u64; 8],
        packets: [u64; 3],
        committed: Vec<u64>,
    }

    fn complete_observation(world: &World) -> CompleteObservation {
        let stats = world.stats;
        CompleteObservation {
            references: world
                .oracle
                .refs
                .iter()
                .map(|chain| {
                    chain
                        .iter()
                        .map(|(&height, block)| Reference {
                            height,
                            hash: block.bh,
                            result: block.result,
                            header: block.header.clone(),
                            at: block.at,
                            honest: block.honest_proposer,
                            view: block.view,
                        })
                        .collect()
                })
                .collect(),
            certificates: world
                .replicas
                .iter()
                .map(|replica| {
                    replica
                        .store
                        .iter()
                        .map(|(body, qc)| Certificate {
                            header: body.header().encode(),
                            qc: qc.encode(),
                            availability: body.availability().as_slice().to_vec(),
                            payload: body.payload().as_slice().to_vec(),
                        })
                        .collect()
                })
                .collect(),
            counters: [
                stats.events,
                stats.bytes,
                stats.lost,
                stats.oversize,
                stats.crashes,
                stats.evidence,
                stats.proposals,
                world.replicas.iter().map(|r| r.host.ingress_drops()).sum(),
            ],
            packets: stats.packets,
            committed: (0..world.replicas.len())
                .map(|r| world.committed(r))
                .collect(),
        }
    }

    fn short_certified_scenario(seed: u64) -> crate::sim::Scenario {
        let mut scenario = scenarios::smoke(seed, 4);
        scenario.duration = 6_000;
        scenario.checks.progress = 2;
        scenario
    }

    #[test]
    fn serial_and_parallel_match_complete_certified_history_and_every_counter() {
        let work = |seed| {
            let world = run(short_certified_scenario(seed)).unwrap_or_else(|e| panic!("{e}"));
            assert!(!world.oracle.refs[0].is_empty());
            assert!(world.replicas.iter().any(|r| !r.store.is_empty()));
            let observation = complete_observation(&world);
            drop(world);
            observation
        };
        let mut serial = Vec::new();
        fold_seed_batches(0..6, 1, work, |seed, observation| {
            serial.push((seed, observation))
        });
        let mut parallel = Vec::new();
        fold_seed_batches(0..6, WORKERS, work, |seed, observation| {
            parallel.push((seed, observation))
        });
        assert_eq!(serial, parallel);
    }

    #[test]
    fn serial_and_parallel_match_original_cpu_flood_certificates_and_every_counter() {
        let work = |seed| {
            let world = run(scenarios::f29(seed)).unwrap_or_else(|e| panic!("{e}"));
            let observation = complete_observation(&world);
            drop(world);
            observation
        };
        let mut serial = Vec::new();
        fold_seed_batches(0..4, 1, work, |seed, observation| {
            serial.push((seed, observation))
        });
        let mut parallel = Vec::new();
        fold_seed_batches(0..4, WORKERS, work, |seed, observation| {
            parallel.push((seed, observation))
        });
        assert_eq!(serial, parallel);
    }

    #[test]
    fn real_worlds_release_original_signing_owners_before_ordered_fold() {
        let live = Arc::new(AtomicUsize::new(0));
        let peak = AtomicUsize::new(0);
        let mut observed = Vec::new();
        fold_seeds(
            0..17,
            |seed| {
                let count = live.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(count, Ordering::SeqCst);
                let owner = Live(Arc::clone(&live));
                let world = run(short_certified_scenario(seed)).unwrap_or_else(|e| panic!("{e}"));
                let signing_owner = Arc::downgrade(&world.log);
                drop(world);
                drop(owner);
                signing_owner
            },
            |seed, signing_owner| {
                assert!(
                    signing_owner.upgrade().is_none(),
                    "seed {seed} retained its World"
                );
                assert_eq!(live.load(Ordering::SeqCst), 0);
                observed.push(seed);
            },
        );
        assert_eq!(observed, (0..17).collect::<Vec<_>>());
        assert!((1..=WORKERS).contains(&peak.load(Ordering::SeqCst)));
    }
}
