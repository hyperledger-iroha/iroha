//! Bounded, ordered collection for SORA Nexus all-peer observations.

/// Observe every input with at most four workers and return input-ordered results.
///
/// Jobs and results need only `Send`: each peer/client belongs to one worker.
/// Operation errors remain ordinary results for the caller to inspect. A worker
/// panic fails the whole collection after all other workers have joined; it can
/// never yield a successful partial observation set.
pub(super) fn collect_bounded_observations<J, R, F>(
    jobs: Vec<J>,
    stack_size: usize,
    observe: F,
) -> Vec<R>
where
    J: Send,
    R: Send,
    F: Fn(J) -> R + Sync,
{
    let worker_count = jobs.len().min(4);
    if worker_count == 0 {
        return Vec::new();
    }
    let mut batches = (0..worker_count).map(|_| Vec::new()).collect::<Vec<_>>();
    for (ordinal, job) in jobs.into_iter().enumerate() {
        batches[ordinal % worker_count].push((ordinal, job));
    }
    std::thread::scope(|scope| {
        let observe = &observe;
        let workers = batches
            .into_iter()
            .enumerate()
            .map(|(worker, batch)| {
                std::thread::Builder::new()
                    .name(format!("aps-observation-{worker}"))
                    .stack_size(stack_size)
                    .spawn_scoped(scope, move || {
                        batch
                            .into_iter()
                            .map(|(ordinal, job)| (ordinal, observe(job)))
                            .collect::<Vec<_>>()
                    })
                    .expect("all-peer observation worker must start")
            })
            .collect::<Vec<_>>();
        let mut ordered = Vec::new();
        let mut panic = None;
        for worker in workers {
            match worker.join() {
                Ok(results) => ordered.extend(results),
                Err(payload) => {
                    if panic.is_none() {
                        panic = Some(payload);
                    }
                }
            }
        }
        if let Some(payload) = panic {
            std::panic::resume_unwind(payload);
        }
        ordered.sort_unstable_by_key(|(ordinal, _)| *ordinal);
        ordered.into_iter().map(|(_, result)| result).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::collect_bounded_observations;
    use std::{
        cell::Cell,
        collections::BTreeSet,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
            mpsc,
        },
        time::Duration,
    };

    const STACK_BYTES: usize = 2 * 1024 * 1024;

    #[test]
    fn all_sixteen_results_and_multiple_errors_keep_input_order() {
        let seen = Mutex::new(Vec::new());
        let results = collect_bounded_observations((0..16).collect(), STACK_BYTES, |peer| {
            seen.lock().unwrap().push(peer);
            if [2, 11, 15].contains(&peer) {
                Err(format!("peer-{peer}: request failed"))
            } else {
                Ok(peer)
            }
        });
        assert_eq!(results.len(), 16);
        for (peer, result) in results.into_iter().enumerate() {
            if [2, 11, 15].contains(&peer) {
                assert_eq!(result, Err(format!("peer-{peer}: request failed")));
            } else {
                assert_eq!(result, Ok(peer));
            }
        }
        let mut seen = seen.into_inner().unwrap();
        seen.sort_unstable();
        assert_eq!(seen, (0..16).collect::<Vec<_>>());
    }

    #[test]
    fn four_live_workers_overlap_and_join_before_return() {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx): (Vec<_>, Vec<_>) =
            (0..4).map(|_| mpsc::channel::<()>()).unzip();
        let mut releases = release_rx.into_iter();
        let jobs = (0..16)
            .map(|peer| (peer, (peer < 4).then(|| releases.next().unwrap())))
            .collect();
        let active = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        let seen_threads = Mutex::new(BTreeSet::new());
        std::thread::scope(|scope| {
            let collector = scope.spawn(|| {
                collect_bounded_observations(jobs, STACK_BYTES, |(peer, release)| {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    seen_threads
                        .lock()
                        .unwrap()
                        .insert(std::thread::current().name().unwrap().to_owned());
                    if let Some(release) = release {
                        started_tx.send(peer).unwrap();
                        release.recv_timeout(Duration::from_secs(10)).unwrap();
                    }
                    active.fetch_sub(1, Ordering::SeqCst);
                    peer
                })
            });
            let mut started = (0..4)
                .map(|_| started_rx.recv_timeout(Duration::from_secs(10)).unwrap())
                .collect::<Vec<_>>();
            started.sort_unstable();
            assert_eq!(started, [0, 1, 2, 3]);
            assert_eq!(active.load(Ordering::SeqCst), 4);
            for release in release_tx.into_iter().rev() {
                release.send(()).unwrap();
            }
            assert_eq!(collector.join().unwrap(), (0..16).collect::<Vec<_>>());
        });
        assert_eq!(peak.load(Ordering::SeqCst), 4);
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert_eq!(seen_threads.into_inner().unwrap().len(), 4);
    }

    #[test]
    fn empty_and_small_collections_do_not_pad_the_peer_set() {
        for count in 0..=3 {
            let calls = AtomicUsize::new(0);
            let results = collect_bounded_observations((0..count).collect(), STACK_BYTES, |peer| {
                calls.fetch_add(1, Ordering::Relaxed);
                peer
            });
            assert_eq!(results, (0..count).collect::<Vec<_>>());
            assert_eq!(calls.load(Ordering::Relaxed), count);
        }
    }

    #[test]
    fn owned_jobs_and_results_do_not_require_sync() {
        let values = (0..16).map(Cell::new).collect();
        let results = collect_bounded_observations(values, STACK_BYTES, |value| {
            value.set(value.get() + 1);
            value
        });
        assert_eq!(
            results
                .into_iter()
                .map(Cell::into_inner)
                .collect::<Vec<_>>(),
            (1..=16).collect::<Vec<_>>()
        );
    }

    #[test]
    fn worker_panic_is_not_a_partial_success_and_other_workers_are_joined() {
        let completed = AtomicUsize::new(0);
        let result = std::panic::catch_unwind(|| {
            collect_bounded_observations((0..16).collect(), STACK_BYTES, |peer| {
                assert_ne!(peer, 0, "injected worker panic");
                completed.fetch_add(1, Ordering::SeqCst);
                peer
            })
        });
        assert!(result.is_err());
        // Worker zero stops; all four jobs on each other worker finish first.
        assert_eq!(completed.load(Ordering::SeqCst), 12);
    }

    #[test]
    fn repeated_rounds_requery_every_peer_without_reusing_prior_results() {
        let per_peer = (0..16).map(|_| AtomicUsize::new(0)).collect::<Vec<_>>();
        for round in 1..=3 {
            let results = collect_bounded_observations((0..16).collect(), STACK_BYTES, |peer| {
                (peer, per_peer[peer].fetch_add(1, Ordering::SeqCst) + 1)
            });
            assert_eq!(
                results,
                (0..16).map(|peer| (peer, round)).collect::<Vec<_>>()
            );
        }
    }
}
