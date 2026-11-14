//! Bounded concurrent dispatch of the independent per-leg committee work of one phase.

use eyre::{Result, eyre};

/// Greatest number of legs whose committee work is in flight together.
///
/// One leg addresses one four-validator committee, so one phase of one operation has at
/// most sixteen committee requests outstanding, whatever the bundle's leg count.
pub(super) const PRIVATE_SETTLEMENT_LEG_FANOUT_V1: usize = 4;

/// Run `work` once per leg, at most [`PRIVATE_SETTLEMENT_LEG_FANOUT_V1`] legs together.
///
/// Legs are taken in leg-ordered windows. Every leg started in a window joins before any
/// of its results is used and before the next window starts. Results are then read in leg
/// order, exactly as a leg-by-leg loop would produce them: the first leg that failed
/// returns its error, the first leg that is not complete yet (`Ok(None)`) returns
/// `Ok(None)`, and no later window starts. Legs of the deciding window that follow the
/// deciding leg have already run; their results are discarded.
///
/// # Errors
/// The first failed leg's error in leg order, or a leg worker that could not start.
///
/// # Panics
/// Resumes a leg's own panic once that leg is the first in leg order not to complete.
pub(super) fn collect_private_settlement_leg_results_v1<T, F>(
    legs: usize,
    work: F,
) -> Result<Option<Vec<T>>>
where
    T: Send,
    F: Fn(usize) -> Result<Option<T>> + Sync,
{
    collect_leg_results_within(PRIVATE_SETTLEMENT_LEG_FANOUT_V1, legs, work)
}

/// [`collect_private_settlement_leg_results_v1`] for work that always completes a leg.
///
/// # Errors
/// The first failed leg's error in leg order, or a leg worker that could not start.
pub(super) fn collect_private_settlement_legs_v1<T, F>(legs: usize, work: F) -> Result<Vec<T>>
where
    T: Send,
    F: Fn(usize) -> Result<T> + Sync,
{
    collect_private_settlement_leg_results_v1(legs, |leg| work(leg).map(Some))?
        .ok_or_else(|| eyre!("private-settlement leg result is incomplete"))
}

fn collect_leg_results_within<T, F>(width: usize, legs: usize, work: F) -> Result<Option<Vec<T>>>
where
    T: Send,
    F: Fn(usize) -> Result<Option<T>> + Sync,
{
    let mut complete = Vec::with_capacity(legs);
    let mut first = 0;
    while first < legs {
        let last = legs.min(first + width.max(1));
        let joined = std::thread::scope(|scope| {
            let work = &work;
            // Start the whole window before joining any of it: a lazily driven iterator
            // would join each leg before starting the next and run the legs one by one.
            let mut started = Vec::with_capacity(last - first);
            for leg in first..last {
                started.push(
                    std::thread::Builder::new()
                        .name(format!("aps-leg-{leg}"))
                        .spawn_scoped(scope, move || work(leg)),
                );
            }
            // Collecting into a Vec completes every join before any result is read. Do
            // not fuse this into an iterator that could return early and leave an
            // initiated leg, with its own committee requests, running.
            started
                .into_iter()
                .map(|child| child.map(std::thread::ScopedJoinHandle::join))
                .collect::<Vec<_>>()
        });
        for outcome in joined {
            let result = match outcome {
                Err(_) => return Err(eyre!("private-settlement leg worker could not start")),
                // The panic stays a panic, as it was when legs ran on the calling thread.
                Ok(Err(panic)) => std::panic::resume_unwind(panic),
                Ok(Ok(result)) => result?,
            };
            let Some(value) = result else {
                return Ok(None);
            };
            complete.push(value);
        }
        first = last;
    }
    Ok(Some(complete))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::BTreeSet,
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{
            Condvar, Mutex,
            atomic::{AtomicUsize, Ordering},
            mpsc,
        },
        time::Duration,
    };

    const TIMEOUT: Duration = Duration::from_secs(10);

    /// Counts legs inside `work` and lets a leg return only when `gate` legs entered.
    struct Overlap {
        gates: Vec<usize>,
        patience: Duration,
        // Legs entered so far, legs inside `work` now, and the widest overlap seen.
        state: Mutex<(usize, usize, usize)>,
        changed: Condvar,
    }
    impl Overlap {
        fn new(gates: &[usize], patience: Duration) -> Self {
            Self {
                gates: gates.to_vec(),
                patience,
                state: Mutex::new((0, 0, 0)),
                changed: Condvar::new(),
            }
        }
        // A bounded gate makes leg-by-leg dispatch fail instead of deadlocking the suite.
        fn pass(&self, leg: usize) -> Result<()> {
            let mut state = self.state.lock().expect("overlap state");
            state.0 += 1;
            state.1 += 1;
            state.2 = state.2.max(state.1);
            self.changed.notify_all();
            let gate = self.gates[leg];
            let (mut state, _) = self
                .changed
                .wait_timeout_while(state, self.patience, |state| state.0 < gate)
                .expect("overlap wait");
            state.1 -= 1;
            if state.0 < gate {
                return Err(eyre!("leg {leg} ran without its window"));
            }
            Ok(())
        }
        fn widest(&self) -> usize {
            self.state.lock().expect("overlap state").2
        }
    }

    #[test]
    fn leg_fanout_overlaps_one_window_and_restores_leg_order() {
        // Six legs: the first four must be in flight together, then the last two.
        let overlap = Overlap::new(&[4, 4, 4, 4, 6, 6], TIMEOUT);
        let (finished_tx, finished_rx) = mpsc::channel();
        let values = collect_private_settlement_legs_v1(6, |leg| {
            overlap.pass(leg)?;
            // Complete each window in reverse leg order.
            std::thread::sleep(Duration::from_millis([15, 10, 5, 0][leg % 4]));
            finished_tx.send(leg).expect("completion log");
            Ok(leg * 10)
        })
        .expect("every leg of a window overlaps the others");
        assert_eq!(
            values,
            vec![0, 10, 20, 30, 40, 50],
            "results keep leg order"
        );
        assert_eq!(
            overlap.widest(),
            PRIVATE_SETTLEMENT_LEG_FANOUT_V1,
            "never more than one window of legs in flight"
        );
        let finished = finished_rx.try_iter().collect::<Vec<_>>();
        assert_eq!(
            finished[..4].iter().copied().collect::<BTreeSet<_>>(),
            BTreeSet::from([0, 1, 2, 3]),
            "the whole first window joined before the second window started"
        );
        assert_eq!(
            finished[4..].iter().copied().collect::<BTreeSet<_>>(),
            BTreeSet::from([4, 5])
        );
    }

    #[test]
    fn one_leg_at_a_time_cannot_satisfy_the_overlap_gate() {
        // A width of one is the former leg-by-leg loop: the same gate refuses it.
        let overlap = Overlap::new(&[2, 2], Duration::from_millis(200));
        let error = collect_leg_results_within(1, 2, |leg| overlap.pass(leg).map(Some))
            .expect_err("sequential legs never overlap");
        assert!(error.to_string().contains("leg 0 ran without its window"));
        assert_eq!(overlap.widest(), 1);
        let overlap = Overlap::new(&[2, 2], TIMEOUT);
        collect_leg_results_within(2, 2, |leg| overlap.pass(leg).map(Some))
            .expect("two legs overlap")
            .expect("both complete");
        assert_eq!(overlap.widest(), 2);
    }

    #[test]
    fn leg_fanout_joins_every_started_leg_before_the_first_leg_ordered_refusal() {
        struct Finished<'a>(&'a AtomicUsize, usize);
        impl Drop for Finished<'_> {
            fn drop(&mut self) {
                self.0.fetch_or(1 << self.1, Ordering::SeqCst);
            }
        }
        let finished = AtomicUsize::new(0);
        let started = AtomicUsize::new(0);
        let error = collect_private_settlement_leg_results_v1(6, |leg| {
            started.fetch_or(1 << leg, Ordering::SeqCst);
            let _finished = Finished(&finished, leg);
            // The refused legs answer first; the others are still running.
            if leg != 1 && leg != 2 {
                std::thread::sleep(Duration::from_millis(50));
            }
            match leg {
                1 => Err(eyre!("leg one refused")),
                2 => Err(eyre!("leg two refused")),
                _ => Ok(Some(leg)),
            }
        })
        .expect_err("a refused leg fails the phase");
        assert!(
            error.to_string().contains("leg one refused"),
            "the first refusal in leg order is returned: {error}"
        );
        assert_eq!(
            finished.load(Ordering::SeqCst),
            0b1111,
            "a returned error must not leave any started leg running"
        );
        assert_eq!(
            started.load(Ordering::SeqCst),
            0b1111,
            "no later window starts after a refusal"
        );
    }

    #[test]
    fn leg_fanout_reads_pending_and_refused_legs_in_leg_order() {
        // An incomplete leg before a refused one reports "not complete yet".
        let pending = collect_private_settlement_leg_results_v1(3, |leg| match leg {
            0 => Ok(Some(leg)),
            1 => Ok(None),
            _ => Err(eyre!("later refusal")),
        })
        .expect("the earlier incomplete leg decides");
        assert_eq!(pending, None);
        // A refused leg before an incomplete one reports the refusal.
        let error = collect_private_settlement_leg_results_v1(3, |leg| match leg {
            0 => Ok(Some(leg)),
            1 => Err(eyre!("earlier refusal")),
            _ => Ok(None),
        })
        .expect_err("the earlier refusal decides");
        assert!(error.to_string().contains("earlier refusal"));
        // An incomplete leg stops later windows.
        let started = AtomicUsize::new(0);
        let pending = collect_private_settlement_leg_results_v1(9, |leg| {
            started.fetch_add(1, Ordering::SeqCst);
            Ok((leg != 3).then_some(leg))
        })
        .expect("no leg failed");
        assert_eq!(pending, None);
        assert_eq!(started.load(Ordering::SeqCst), 4);
        assert_eq!(
            collect_private_settlement_legs_v1(0, Ok).expect("no legs"),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn leg_fanout_resumes_the_first_leg_ordered_panic_after_joining_every_leg() {
        let finished = AtomicUsize::new(0);
        let unwound = catch_unwind(AssertUnwindSafe(|| {
            collect_private_settlement_legs_v1(3, |leg| {
                if leg == 1 {
                    panic!("intentional leg-worker panic");
                }
                std::thread::sleep(Duration::from_millis(50));
                finished.fetch_or(1 << leg, Ordering::SeqCst);
                Ok(leg)
            })
        }))
        .expect_err("a leg panic stays a panic");
        assert_eq!(
            unwound.downcast_ref::<&str>().copied(),
            Some("intentional leg-worker panic")
        );
        assert_eq!(
            finished.load(Ordering::SeqCst),
            0b101,
            "the other legs were joined before the panic resumed"
        );
        // An earlier ordinary refusal outranks a later panic.
        let error = collect_private_settlement_legs_v1(2, |leg| {
            if leg == 1 {
                panic!("intentional later leg-worker panic");
            }
            Err::<usize, _>(eyre!("earlier refusal"))
        })
        .expect_err("the earlier refusal decides");
        assert!(error.to_string().contains("earlier refusal"));
    }
}
