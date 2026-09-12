/// Replica fetch admission and physical-worker settlement regressions.
mod replica_fetch_boundary_tests {
    use std::sync::{Condvar, mpsc};

    use super::*;

    const WAIT_LIMIT: Duration = Duration::from_secs(5);

    enum FixedReply<'a> {
        Bytes { physical: &'a [u8], reported: usize },
        Error(SccpReplayCheckpointSourceErrorV1),
    }

    impl SccpReplayCheckpointSourceV1 for FixedReply<'_> {
        fn fetch_to(
            &self,
            _replica: &ToriiSccpReplayArchiveReplica,
            _max_response_bytes: usize,
            _timeout: Duration,
            destination: &mut dyn std::io::Write,
        ) -> Result<usize, SccpReplayCheckpointSourceErrorV1> {
            match self {
                Self::Bytes { physical, reported } => {
                    destination
                        .write_all(physical)
                        .map_err(|_| SccpReplayCheckpointSourceErrorV1::Transport)?;
                    Ok(*reported)
                }
                Self::Error(error) => Err(*error),
            }
        }
    }

    #[test]
    fn real_signed_replica_fetch_accepts_the_exact_response_ceiling() {
        let fixture = Fixture::new();
        let service = fixture
            .bootstrap()
            .expect("real signed fixture authenticates");
        assert_eq!(
            service.checkpoint_set_sha256(),
            Ok(sccp_replay_archive_checkpoint_set_frame_sha256_v1(
                &fixture.first_bytes
            ))
        );
        drop(service);
        let store = SecureReplayStoreV1::open(&fixture.config.state_dir)
            .expect("the same private store reopens");
        let mut config = fixture.config.clone();
        config.max_response_bytes = fixture.first_bytes.len();
        let source = FixedReply::Bytes {
            physical: &fixture.first_bytes,
            reported: fixture.first_bytes.len(),
        };
        assert_eq!(
            fetch_exact_three(&config, &source, &store),
            Ok(fixture.first_bytes.clone())
        );
        config.max_response_bytes -= 1;
        assert_eq!(
            fetch_exact_three(&config, &source, &store),
            Err(ToriiSccpReplayStartupErrorV1::Transport),
            "the adapter ignores the ceiling, so the owner must enforce it"
        );
    }

    #[test]
    fn zero_and_oversized_reported_lengths_fail_before_descriptor_comparison() {
        let fixture = Fixture::new();
        let store = SecureReplayStoreV1::open(&fixture.config.state_dir)
            .expect("private fixture store opens");
        let mut config = fixture.config.clone();
        config.max_response_bytes = 4;
        let cases: &[(&[u8], usize)] = &[(&[], 0), (&[7], 0), (&[7; 5], 5), (&[7], usize::MAX)];
        for &(physical, reported) in cases {
            assert_eq!(
                fetch_exact_three(&config, &FixedReply::Bytes { physical, reported }, &store),
                Err(ToriiSccpReplayStartupErrorV1::Transport),
                "owner length rejection precedes mismatched-file validation"
            );
        }
    }

    #[test]
    fn in_range_reported_and_physical_length_mismatch_remains_persistence() {
        let fixture = Fixture::new();
        let store = SecureReplayStoreV1::open(&fixture.config.state_dir)
            .expect("private fixture store opens");
        for (physical, reported) in [(&[7][..], 2), (&[7; 2][..], 1)] {
            assert_eq!(
                fetch_exact_three(
                    &fixture.config,
                    &FixedReply::Bytes { physical, reported },
                    &store
                ),
                Err(ToriiSccpReplayStartupErrorV1::Persistence)
            );
        }
    }

    #[test]
    fn all_current_source_errors_remain_transport_failures() {
        let fixture = Fixture::new();
        let store = SecureReplayStoreV1::open(&fixture.config.state_dir)
            .expect("private fixture store opens");
        for error in [
            SccpReplayCheckpointSourceErrorV1::Limit,
            SccpReplayCheckpointSourceErrorV1::Protocol,
            SccpReplayCheckpointSourceErrorV1::Transport,
        ] {
            assert_eq!(
                fetch_exact_three(&fixture.config, &FixedReply::Error(error), &store),
                Err(ToriiSccpReplayStartupErrorV1::Transport)
            );
        }
    }

    struct CountOnDrop<'a>(&'a AtomicUsize);

    impl Drop for CountOnDrop<'_> {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[derive(Clone, Copy)]
    enum ReplicaFailure {
        Panic,
        Error,
    }

    struct ConcurrentSource<'a> {
        bytes: &'a [u8],
        ids: [[u8; 32]; 3],
        failure: Option<ReplicaFailure>,
        arrivals: Mutex<usize>,
        changed: Condvar,
        seen: AtomicUsize,
        suppressed: AtomicUsize,
        settled: AtomicUsize,
        timed_out: AtomicBool,
    }

    impl SccpReplayCheckpointSourceV1 for ConcurrentSource<'_> {
        fn fetch_to(
            &self,
            replica: &ToriiSccpReplayArchiveReplica,
            _max_response_bytes: usize,
            _timeout: Duration,
            destination: &mut dyn std::io::Write,
        ) -> Result<usize, SccpReplayCheckpointSourceErrorV1> {
            let _settled = CountOnDrop(&self.settled);
            let index = self
                .ids
                .iter()
                .position(|id| id == &replica.replica_id)
                .ok_or(SccpReplayCheckpointSourceErrorV1::Protocol)?;
            self.seen.fetch_or(1 << index, Ordering::SeqCst);
            if iroha_core::panic_hook::is_suppressed() {
                self.suppressed.fetch_add(1, Ordering::SeqCst);
            }
            let mut arrivals = self
                .arrivals
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            *arrivals += 1;
            self.changed.notify_all();
            let (arrivals, _) = self
                .changed
                .wait_timeout_while(arrivals, WAIT_LIMIT, |count| *count < 3)
                .unwrap_or_else(|error| error.into_inner());
            let all_entered = *arrivals == 3;
            drop(arrivals);
            if !all_entered {
                self.timed_out.store(true, Ordering::SeqCst);
                return Err(SccpReplayCheckpointSourceErrorV1::Transport);
            }
            if index == 0 {
                match self.failure {
                    Some(ReplicaFailure::Panic) => panic!("injected replica callback panic"),
                    Some(ReplicaFailure::Error) => {
                        return Err(SccpReplayCheckpointSourceErrorV1::Protocol);
                    }
                    None => {}
                }
            }
            destination
                .write_all(self.bytes)
                .map_err(|_| SccpReplayCheckpointSourceErrorV1::Transport)?;
            Ok(self.bytes.len())
        }
    }

    fn assert_concurrent_fetch(failure: Option<ReplicaFailure>) {
        let fixture = Fixture::new();
        let store = SecureReplayStoreV1::open(&fixture.config.state_dir)
            .expect("private fixture store opens");
        let source = ConcurrentSource {
            bytes: &fixture.first_bytes,
            ids: core::array::from_fn(|index| fixture.config.replicas[index].replica_id),
            failure,
            arrivals: Mutex::new(0),
            changed: Condvar::new(),
            seen: AtomicUsize::new(0),
            suppressed: AtomicUsize::new(0),
            settled: AtomicUsize::new(0),
            timed_out: AtomicBool::new(false),
        };
        let result = fetch_exact_three(&fixture.config, &source, &store);
        if failure.is_some() {
            assert_eq!(result, Err(ToriiSccpReplayStartupErrorV1::Transport));
        } else {
            assert_eq!(result, Ok(fixture.first_bytes.clone()));
        }
        assert!(!source.timed_out.load(Ordering::SeqCst));
        assert_eq!(source.seen.load(Ordering::SeqCst), 0b111);
        assert_eq!(
            *source
                .arrivals
                .lock()
                .expect("all callbacks released the lock"),
            3
        );
        assert_eq!(source.suppressed.load(Ordering::SeqCst), 3);
        assert_eq!(source.settled.load(Ordering::SeqCst), 3);
        assert!(!iroha_core::panic_hook::is_suppressed());
    }

    #[test]
    fn all_three_replica_callbacks_overlap_under_suppression() {
        assert_concurrent_fetch(None);
    }

    #[test]
    fn replica_callback_panic_and_error_settle_all_three_workers() {
        assert_concurrent_fetch(Some(ReplicaFailure::Panic));
        assert_concurrent_fetch(Some(ReplicaFailure::Error));
    }

    struct ReleaseOnDrop(mpsc::SyncSender<()>);

    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            let _ = self.0.try_send(());
        }
    }

    #[derive(Clone, Copy)]
    enum WorkerFailure {
        Creation,
        Panic,
        Domain,
    }

    fn assert_join_settles_every_worker(failure: WorkerFailure) {
        let fixture = Fixture::new();
        let store = SecureReplayStoreV1::open(&fixture.config.state_dir)
            .expect("private fixture store opens");
        let completed = AtomicUsize::new(0);
        let visited = AtomicUsize::new(0);
        let timed_out = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let (release, wait) = mpsc::sync_channel(1);
            let _release_on_unwind = ReleaseOnDrop(release.clone());
            let first_file = store
                .create_anonymous_fetch_file()
                .expect("first file opens");
            let completed_ref = &completed;
            let first = std::thread::Builder::new()
                .spawn_scoped(scope, move || {
                    let _completed = CountOnDrop(completed_ref);
                    Ok((first_file, 0))
                })
                .expect("first test worker starts");
            let middle = match failure {
                WorkerFailure::Creation => Err(std::io::Error::other("injected creation failure")),
                WorkerFailure::Panic | WorkerFailure::Domain => std::thread::Builder::new()
                    .spawn_scoped(scope, move || {
                        let _completed = CountOnDrop(completed_ref);
                        if matches!(failure, WorkerFailure::Panic) {
                            iroha_core::panic_hook::with_hook_suppressed(|| {
                                panic!("injected joined worker panic")
                            });
                        }
                        Err(ToriiSccpReplayStartupErrorV1::Persistence)
                    }),
            };
            let last_file = store
                .create_anonymous_fetch_file()
                .expect("last file opens");
            let timed_out_ref = &timed_out;
            let last = std::thread::Builder::new()
                .spawn_scoped(scope, move || {
                    let _completed = CountOnDrop(completed_ref);
                    if wait.recv_timeout(WAIT_LIMIT).is_err() {
                        timed_out_ref.store(true, Ordering::SeqCst);
                        return Err(ToriiSccpReplayStartupErrorV1::Persistence);
                    }
                    Ok((last_file, 0))
                })
                .expect("last test worker starts");
            let workers =
                [Ok(first), middle, Ok(last)]
                    .into_iter()
                    .enumerate()
                    .map(|(index, worker)| {
                        visited.fetch_add(1, Ordering::SeqCst);
                        if index == 2 {
                            let _ = release.try_send(());
                        }
                        worker
                    });
            let result = join_replica_fetch_workers(workers).map(|_| ());
            let expected = match failure {
                WorkerFailure::Domain => ToriiSccpReplayStartupErrorV1::Persistence,
                WorkerFailure::Creation | WorkerFailure::Panic => {
                    ToriiSccpReplayStartupErrorV1::Transport
                }
            };
            assert_eq!(result, Err(expected));
            assert_eq!(visited.load(Ordering::SeqCst), 3);
            assert!(!timed_out.load(Ordering::SeqCst));
            assert_eq!(
                completed.load(Ordering::SeqCst),
                if matches!(failure, WorkerFailure::Creation) {
                    2
                } else {
                    3
                },
                "all started work is settled before the enclosing scope can auto-join"
            );
        });
        assert!(!iroha_core::panic_hook::is_suppressed());
    }

    #[test]
    fn creation_failure_visits_and_joins_successful_workers_on_both_sides() {
        assert_join_settles_every_worker(WorkerFailure::Creation);
    }

    #[test]
    fn join_panic_visits_and_joins_every_other_worker() {
        assert_join_settles_every_worker(WorkerFailure::Panic);
    }

    #[test]
    fn domain_failure_visits_and_joins_every_other_worker() {
        assert_join_settles_every_worker(WorkerFailure::Domain);
    }
}
