//! The carrier barrier overlaps only the first read-only promoted-catalog check. Every write
//! keeps the serial recipe's order, and every failure pattern returns the serial failure.

use super::*;
use crate::managed::runtime::progress::Cause;
use std::sync::{Mutex, mpsc};

const WAIT: Duration = Duration::from_secs(10);
const BARRIER: u8 = 10;
const CHECK: [u8; 3] = [20, 21, 22];
const ADVERTISE: [u8; 3] = [30, 31, 32];

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

fn refusal(code: u8) -> Failure {
    Failure::ValidatorExited { index: code }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Barrier,
    Check(usize),
    Advertise(usize),
    Observe,
}

/// Which steps refuse. Each refusal is distinct, so the reported one is identifiable.
#[derive(Clone, Copy, Debug)]
struct Faults {
    barrier: bool,
    check: [bool; 3],
    advertise: [bool; 3],
}
impl Faults {
    fn all() -> impl Iterator<Item = Self> {
        (0_u8..128).map(|bits| Self {
            barrier: bits & 1 != 0,
            check: std::array::from_fn(|index| bits & (2 << index) != 0),
            advertise: std::array::from_fn(|index| bits & (16 << index) != 0),
        })
    }
}

type Outcome = (std::result::Result<[usize; 3], Failure>, Vec<Step>);

/// The pre-overlap recipe: the whole barrier, then each provider's check and advertisement.
fn serial(faults: Faults) -> Outcome {
    let mut log = Vec::new();
    let result = (|| {
        log.push(Step::Barrier);
        if faults.barrier {
            return Err(refusal(BARRIER));
        }
        let refusals = faults.check.into_iter().zip(faults.advertise);
        for (index, (check, advertise)) in refusals.enumerate() {
            log.push(Step::Check(index));
            if check {
                return Err(refusal(CHECK[index]));
            }
            log.push(Step::Advertise(index));
            if advertise {
                return Err(refusal(ADVERTISE[index]));
            }
        }
        log.push(Step::Observe);
        Ok([0, 1, 2])
    })();
    (result, log)
}

fn overlapped(faults: Faults, budget: &Budget) -> Outcome {
    let log = Mutex::new(Vec::new());
    let mut gateways = [0_usize, 1, 2];
    let result = observe_published_gateways(
        &mut gateways,
        budget,
        |_| {
            log.lock().unwrap().push(Step::Barrier);
            if faults.barrier {
                Err(refusal(BARRIER))
            } else {
                Ok(())
            }
        },
        |index, provider| {
            assert_eq!(index, *provider);
            log.lock().unwrap().push(Step::Check(index));
            if faults.check[index] {
                Err(refusal(CHECK[index]))
            } else {
                Ok(index)
            }
        },
        |provider| {
            log.lock().unwrap().push(Step::Advertise(*provider));
            if faults.advertise[*provider] {
                Err(refusal(ADVERTISE[*provider]))
            } else {
                Ok(())
            }
        },
        |_, floors| {
            log.lock().unwrap().push(Step::Observe);
            Ok(floors)
        },
    );
    (result, log.into_inner().unwrap())
}

fn writes(steps: &[Step]) -> Vec<Step> {
    steps
        .iter()
        .copied()
        .filter(|step| matches!(step, Step::Advertise(_) | Step::Observe))
        .collect()
}

#[test]
fn every_failure_pattern_keeps_the_serial_failure_and_write_order() {
    for faults in Faults::all() {
        let (expected, serial_steps) = serial(faults);
        let (result, steps) = overlapped(faults, &budget());
        assert_eq!(result, expected, "{faults:?}");
        assert_eq!(writes(&steps), writes(&serial_steps), "{faults:?}");
        // The only extra step is the overlapped read-only check after a refused barrier.
        let mut extra = steps.clone();
        for step in &serial_steps {
            let position = extra.iter().position(|seen| seen == step).unwrap();
            extra.remove(position);
        }
        let allowed: &[Step] = if faults.barrier {
            &[Step::Check(0)]
        } else {
            &[]
        };
        assert!(
            extra.iter().all(|step| allowed.contains(step)),
            "{faults:?}"
        );
        // No advertisement precedes the barrier or its own provider's check.
        for index in 0..3 {
            if let Some(advert) = steps.iter().position(|s| *s == Step::Advertise(index)) {
                let check = steps.iter().position(|s| *s == Step::Check(index)).unwrap();
                let barrier = steps.iter().position(|s| *s == Step::Barrier).unwrap();
                assert!(check < advert && barrier < advert, "{faults:?}");
            }
        }
    }
}

fn limited<R>(run: impl FnOnce() -> R) -> R {
    let limit = 64 * 1024 * 1024;
    norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
        limit, limit, limit, limit, 64,
    ))
    .with(run)
}

#[test]
fn active_decode_limits_keep_the_exact_serial_recipe() {
    for faults in Faults::all() {
        let outcome = limited(|| overlapped(faults, &budget()));
        assert_eq!(outcome, serial(faults), "{faults:?}");
    }
    let caller = thread::current().id();
    let checked = AtomicBool::new(false);
    let mut gateways = [0_usize, 1, 2];
    let result = limited(|| {
        observe_published_gateways(
            &mut gateways,
            &budget(),
            |_| {
                assert_eq!(thread::current().id(), caller);
                assert!(!checked.load(Ordering::Acquire));
                Ok(())
            },
            |_, _| {
                checked.store(true, Ordering::Release);
                Ok(())
            },
            |_| Ok(()),
            |_, floors| Ok(floors),
        )
    });
    assert!(result.is_ok());
}

#[test]
fn barrier_runs_beside_the_first_check_with_forked_phases_and_shared_timers() {
    let budget = budget();
    budget.progress.enter(Phase::Receipt);
    let caller = thread::current().id();
    let (barrier_started, barrier_seen) = mpsc::channel();
    let (check_started, check_seen) = mpsc::channel::<()>();
    let check_seen = Mutex::new(check_seen);
    let mut gateways = [0_usize, 1, 2];
    let result = observe_published_gateways(
        &mut gateways,
        &budget,
        |barrier| {
            assert_ne!(thread::current().id(), caller);
            assert!(Arc::ptr_eq(&barrier.cancelled, &budget.cancelled));
            assert_eq!(
                (barrier.started, barrier.timeout),
                (budget.started, budget.timeout)
            );
            // The fork starts at the caller's phase and keeps later phases to itself.
            assert_eq!(
                barrier.progress.unconfirmed(),
                Failure::Activation {
                    phase: Phase::Receipt,
                    cause: Cause::Unconfirmed,
                }
            );
            barrier.progress.enter(Phase::Carrier2);
            barrier_started.send(()).unwrap();
            check_seen
                .lock()
                .unwrap()
                .recv_timeout(WAIT)
                .expect("the first check must run beside the barrier");
            Err(barrier.progress.unconfirmed())
        },
        |index, _| {
            assert_eq!((index, thread::current().id()), (0, caller));
            barrier_seen
                .recv_timeout(WAIT)
                .expect("the barrier must run beside the first check");
            assert_eq!(
                budget.progress.unconfirmed(),
                Failure::Activation {
                    phase: Phase::CarrierPeers,
                    cause: Cause::Unconfirmed,
                }
            );
            check_started.send(()).unwrap();
            Ok(())
        },
        |_| panic!("no advertisement after a refused barrier"),
        |_, _| -> std::result::Result<[(); 3], Failure> { panic!("no observation") },
    );
    // The serial barrier entered the same phase on the shared progress before refusing.
    assert_eq!(
        result,
        Err(Failure::Activation {
            phase: Phase::Carrier2,
            cause: Cause::Unconfirmed,
        })
    );

    // A refusal before the barrier enters any phase keeps the caller's original phase.
    budget.progress.enter(Phase::Receipt);
    let result = observe_published_gateways(
        &mut gateways,
        &budget,
        |barrier| Err(barrier.progress.unconfirmed()),
        |_, _| Ok(()),
        |_| panic!("no advertisement after a refused barrier"),
        |_, _| -> std::result::Result<[(); 3], Failure> { panic!("no observation") },
    );
    assert_eq!(
        result,
        Err(Failure::Activation {
            phase: Phase::Receipt,
            cause: Cause::Unconfirmed,
        })
    );
}

#[test]
fn first_advertisement_waits_for_a_barrier_that_outlasts_the_check() {
    let (checked, check_seen) = mpsc::channel::<()>();
    let check_seen = Mutex::new(check_seen);
    let finished = AtomicBool::new(false);
    let mut gateways = [0_usize, 1, 2];
    let mut adverts = Vec::new();
    let result = observe_published_gateways(
        &mut gateways,
        &budget(),
        |_| {
            check_seen.lock().unwrap().recv_timeout(WAIT).unwrap();
            thread::sleep(Duration::from_millis(50));
            finished.store(true, Ordering::Release);
            Ok(())
        },
        |index, _| {
            if index == 0 {
                checked.send(()).unwrap();
            }
            Ok(index)
        },
        |provider| {
            assert!(finished.load(Ordering::Acquire));
            adverts.push(*provider);
            Ok(())
        },
        |_, floors| Ok(floors),
    );
    assert_eq!(result, Ok([0, 1, 2]));
    assert_eq!(adverts, [0, 1, 2]);
}

#[test]
fn panicked_barrier_is_a_closed_refusal_without_writes() {
    let mut gateways = [0_usize, 1, 2];
    let checked = AtomicBool::new(false);
    let result = observe_published_gateways(
        &mut gateways,
        &budget(),
        |_| -> std::result::Result<(), Failure> { panic!("carrier barrier fault") },
        |_, _| {
            checked.store(true, Ordering::Release);
            Ok(())
        },
        |_| panic!("no advertisement after a panicked barrier"),
        |_, _| -> std::result::Result<[(); 3], Failure> { panic!("no observation") },
    );
    assert_eq!(
        result,
        Err(Failure::Activation {
            phase: Phase::CarrierPeers,
            cause: Cause::Unconfirmed,
        })
    );
    assert!(checked.load(Ordering::Acquire));
}
