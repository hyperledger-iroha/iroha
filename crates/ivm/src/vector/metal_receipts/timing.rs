//! Explicit qualification receipts captured only after complete synthetic sampling.
//!
//! Samples are public synthetic inputs. Arrays preserve `[pattern][trial]` order,
//! with CPU measured before Metal. Zero means no accepted duration was recorded
//! for that slot, including an elapsed-time refusal; it is never a timed zero.
//! The observer performs no work between individual sample timers and cannot
//! alter sample inputs, results, variance limits, deadlines or profile selection.

use std::{cell::Cell, fmt::Debug};

/// Original fixed sample arrays, including every uncompleted slot on refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::vector) struct Timings {
    /// CPU durations in nanoseconds, indexed by public pattern and trial.
    pub(in crate::vector) cpu_ns: [[u64; 3]; 2],
    /// Metal durations in nanoseconds, indexed by public pattern and trial.
    pub(in crate::vector) metal_ns: [[u64; 3]; 2],
}

#[derive(Clone, Copy)]
enum Slot {
    Inactive,
    Awaiting,
    Captured(Timings),
}

thread_local! {
    static OBSERVATION: Cell<Slot> = const { Cell::new(Slot::Inactive) };
}

/// Publish once, after the sampler has returned, only for an explicit observer.
pub(in crate::vector) fn record(cpu_ns: [[u64; 3]; 2], metal_ns: [[u64; 3]; 2]) {
    OBSERVATION.with(|slot| match slot.get() {
        Slot::Inactive => {}
        Slot::Awaiting => slot.set(Slot::Captured(Timings { cpu_ns, metal_ns })),
        Slot::Captured(_) => panic!("one calibration per timing observation"),
    });
}

/// Scope one observation without allocating, retaining state or changing its result.
pub(in crate::vector) fn observe<T>(call: impl FnOnce() -> T) -> (T, Option<Timings>) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            OBSERVATION.with(|slot| slot.set(Slot::Inactive));
        }
    }
    OBSERVATION.with(|slot| {
        assert!(
            matches!(slot.get(), Slot::Inactive),
            "nested timing observer"
        );
        slot.set(Slot::Awaiting);
    });
    let _reset = Reset;
    let result = call();
    let timings = OBSERVATION.with(|slot| match slot.get() {
        Slot::Captured(timings) => Some(timings),
        Slot::Awaiting => None,
        Slot::Inactive => unreachable!("original timing observer remains active"),
    });
    (result, timings)
}

/// Print after all timers, cleanup and decisions; return the original result.
pub(in crate::vector) fn report<T: Debug>(
    operation: &'static str,
    device: u64,
    geometry: impl Debug,
    baseline: impl Debug,
    call: impl FnOnce() -> T,
) -> T {
    let (result, timings) = observe(call);
    let timings = timings.expect("required calibration must publish its original sample arrays");
    println!(
        "IVM_METAL_CALIBRATION_TIMING operation={operation} device={device} geometry={geometry:?} baseline={baseline:?} cpu_ns={:?} metal_ns={:?} outcome={result:?}",
        timings.cpu_ns, timings.metal_ns,
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_observer_does_not_retain_or_reuse_an_earlier_receipt() {
        record([[1; 3]; 2], [[2; 3]; 2]);
        assert_eq!(observe(|| 7), (7, None));
        let expected = Timings {
            cpu_ns: [[10, 20, 30], [40, 50, 60]],
            metal_ns: [[70, 80, 90], [100, 110, 120]],
        };
        assert_eq!(
            observe(|| record(expected.cpu_ns, expected.metal_ns)),
            ((), Some(expected)),
        );
        assert_eq!(observe(|| ()), ((), None));
    }

    #[test]
    fn refusal_keeps_partial_arrays_and_moves_the_original_error_through_reporting() {
        #[derive(Debug)]
        struct Refusal(Box<u8>);
        let refusal = Refusal(Box::new(5));
        let original = std::ptr::from_ref(refusal.0.as_ref());
        let expected = Timings {
            cpu_ns: [[10, 20, 0], [0; 3]],
            metal_ns: [[30, 0, 0], [0; 3]],
        };
        let (result, receipt) = observe(|| {
            record(expected.cpu_ns, expected.metal_ns);
            Err::<(), _>(refusal)
        });
        assert_eq!(receipt, Some(expected));
        let result = report("Rehash", 17, (8_192, 32, 8_192), "scalar", || {
            record(expected.cpu_ns, expected.metal_ns);
            result
        });
        let returned = result.unwrap_err();
        assert_eq!(std::ptr::from_ref(returned.0.as_ref()), original);
    }

    #[test]
    fn nested_observation_refuses_without_erasing_parent_and_unwind_clears_state() {
        let expected = Timings {
            cpu_ns: [[11; 3]; 2],
            metal_ns: [[22; 3]; 2],
        };
        let (_, receipt) = observe(|| {
            record(expected.cpu_ns, expected.metal_ns);
            assert!(std::panic::catch_unwind(|| observe(|| ())).is_err());
            assert!(std::panic::catch_unwind(|| record([[1; 3]; 2], [[2; 3]; 2])).is_err());
        });
        assert_eq!(receipt, Some(expected));
        assert!(
            std::panic::catch_unwind(|| {
                observe(|| {
                    record(expected.cpu_ns, expected.metal_ns);
                    panic!("calibration unwind");
                });
            })
            .is_err()
        );
        assert_eq!(observe(|| ()), ((), None));
    }
}
