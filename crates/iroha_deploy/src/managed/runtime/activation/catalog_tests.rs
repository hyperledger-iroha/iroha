//! Native lock and deadline controls for scheduling; no owned child or Ready is fabricated.

use super::*;
use crate::managed::service_bootstrap::ManagedServiceBootstrap;
use std::sync::atomic::AtomicUsize;

fn budget() -> Budget {
    Budget {
        started: Instant::now(),
        timeout: Duration::from_secs(120),
        startup_deadline_ns: None,
        utc_ceiling_unix_ms: None,
        cancelled: Arc::new(AtomicBool::new(false)),
        progress: Arc::new(Progress::default()),
    }
}

struct NativeSelection<'a> {
    calls: &'a AtomicUsize,
    refuse: bool,
}
impl LiveGatewayProcess for NativeSelection<'_> {
    fn validate(
        &mut self,
        prepared: &PreparedLocalnet,
        _: &RetainedGatewayCompliancePlan,
    ) -> Result<()> {
        // The callback owns the same real exclusive parent purpose used by full renderer
        // validation. The outer guard must prevent competing callbacks from colliding here.
        let parent = ManagedServiceBootstrap::open(prepared)?;
        assert!(ManagedServiceBootstrap::open(prepared).is_err());
        self.calls.fetch_add(1, Ordering::SeqCst);
        drop(parent);
        if self.refuse {
            Err(invalid("ordinary validation refusal"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn catalog_validation_serializes_original_native_owners_and_releases_on_error() {
    let _guard = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = crate::localnet::prepare_localnet_at(
        "catalog-native-lock",
        &temporary.path().join("generation"),
        &ports,
        LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    let plans = prepared.provider_service_plans().unwrap().unwrap();
    let validation = Mutex::new(());
    let budget = budget();
    let calls = AtomicUsize::new(0);
    for refuse in [false, true, false] {
        let result = crate::managed::provider_round::run(
            [0, 1, 2],
            true,
            |slot| {
                let plan = prepared
                    .gateway_compliance_plan(plans[slot].provider_id())?
                    .unwrap();
                let mut actual = NativeSelection {
                    calls: &calls,
                    refuse: refuse && slot == 0,
                };
                let mut guarded = GuardedGateway {
                    gateway: &mut actual,
                    validation: &validation,
                    budget: &budget,
                };
                guarded.validate(&prepared, &plan)
            },
            || invalid("test worker did not complete"),
        );
        assert_eq!(result.is_err(), refuse);
        assert!(validation.try_lock().is_ok());
        drop(ManagedServiceBootstrap::open(&prepared).unwrap());
    }
    assert_eq!(calls.load(Ordering::SeqCst), 9);
}

#[test]
fn waiting_validation_rejects_original_cancellation_and_expiry_without_entering_callback() {
    let validation = Mutex::new(());
    let held = validation.lock().unwrap();
    let cancelled = budget();
    cancelled.cancelled.store(true, Ordering::Release);
    assert!(acquire_validation(&validation, &cancelled).is_err());
    let expired = Budget {
        started: Instant::now()
            .checked_sub(Duration::from_secs(121))
            .unwrap(),
        ..budget()
    };
    assert!(acquire_validation(&validation, &expired).is_err());
    // Cancellation while a worker waits must also finish without waiting for the holder.
    let waiting = budget();
    thread::scope(|scope| {
        let worker = scope.spawn(|| acquire_validation(&validation, &waiting).map(|_| ()));
        waiting.cancelled.store(true, Ordering::Release);
        assert!(worker.join().unwrap().is_err());
    });
    drop(held);
    assert!(acquire_validation(&validation, &budget()).is_ok());
}

#[test]
fn overlapping_catalog_requests_only_share_a_validation_started_after_their_entry() {
    use std::sync::mpsc;
    let round = LaunchRound::default();
    let budget = budget();
    let reads = AtomicUsize::new(0);
    // Hold the round before selecting requests, then make two waiters eligible
    // for the same physical read. A third starts while that read is in flight.
    let held = round.completed.lock().unwrap();
    let (started, receiving) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let resume = Mutex::new(resume);
    let read = || {
        if reads.fetch_add(1, Ordering::SeqCst) == 0 {
            started.send(()).unwrap();
            resume
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(30))
                .unwrap();
        }
        Ok(())
    };
    let run = || round.validate(&budget, read);
    thread::scope(|scope| {
        let a = scope.spawn(&run);
        let b = scope.spawn(&run);
        let wait = |n| {
            let end = Instant::now() + Duration::from_secs(30);
            while round.requested.load(Ordering::Acquire) < n {
                assert!(Instant::now() < end);
                thread::sleep(Duration::from_millis(1));
            }
        };
        wait(2);
        drop(held);
        receiving.recv_timeout(Duration::from_secs(30)).unwrap();
        let c = scope.spawn(&run);
        wait(3);
        release.send(()).unwrap();
        for worker in [a, b, c] {
            worker.join().unwrap().unwrap();
        }
    });
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    assert!(round.validate(&budget, || Err(invalid("refused"))).is_err());
    assert_eq!(*round.completed.lock().unwrap(), 3);
    round
        .validate(&budget, || {
            reads.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .unwrap();
    assert_eq!(reads.load(Ordering::SeqCst), 3);
    budget.cancelled.store(true, Ordering::Release);
    assert!(
        round
            .validate(&budget, || panic!("cancelled request performed a read"))
            .is_err()
    );
}
