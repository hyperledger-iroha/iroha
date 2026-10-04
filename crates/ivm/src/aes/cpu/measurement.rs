//! Retain the actually exercised CPU baseline across each synthetic sample.

use super::{Backend, Cell, Direction, with_calibration};

// Every ordinary and synthetic round performs the same TLS bit update. There
// is no sampling-only per-round branch or observer callback in the CPU timing.
thread_local! { static OBSERVATION: Cell<u8> = const { Cell::new(0) }; }

fn bit(direction: Direction, native: bool) -> u8 {
    1 << (direction.index() * 2 + usize::from(native))
}

pub(super) fn observe(direction: Direction, native: bool) {
    OBSERVATION.with(|slot| slot.set(slot.get() | bit(direction, native)));
}

/// Every measured round must use the expected path; checking only before/after
/// a sample would miss an opt-out that was enabled and restored within it.
pub(crate) fn measure_backend<T>(
    direction: Direction,
    expected: Backend,
    call: impl FnOnce() -> T,
) -> Option<T> {
    struct Restore(u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            OBSERVATION.with(|slot| slot.set(self.0));
        }
    }
    let _restore = Restore(OBSERVATION.with(|slot| slot.replace(0)));
    let output = with_calibration(call);
    let expected = bit(direction, expected == Backend::Native);
    OBSERVATION
        .with(|slot| slot.get() & !expected == 0)
        .then_some(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_round_must_match_even_when_policy_returns_to_its_initial_value() {
        let original = OBSERVATION.with(Cell::get);
        for (expected, native) in [(Backend::Scalar, false), (Backend::Native, true)] {
            assert_eq!(
                measure_backend(Direction::Encrypt, expected, || {
                    observe(Direction::Encrypt, native);
                    7
                }),
                Some(7)
            );
            assert_eq!(
                measure_backend(Direction::Encrypt, expected, || {
                    observe(Direction::Encrypt, native);
                    observe(Direction::Encrypt, !native);
                    observe(Direction::Encrypt, native);
                    7
                }),
                None
            );
            assert_eq!(
                measure_backend(Direction::Encrypt, expected, || {
                    observe(Direction::Decrypt, native);
                    7
                }),
                None
            );
        }
        assert_eq!(OBSERVATION.with(Cell::get), original);
    }

    #[test]
    fn nested_samples_and_unwind_restore_original_observation() {
        let original = OBSERVATION.with(Cell::get);
        assert_eq!(
            measure_backend(Direction::Encrypt, Backend::Native, || {
                observe(Direction::Encrypt, true);
                assert_eq!(
                    measure_backend(Direction::Decrypt, Backend::Scalar, || {
                        observe(Direction::Decrypt, false);
                    }),
                    Some(())
                );
                assert!(
                    std::panic::catch_unwind(|| measure_backend(
                        Direction::Decrypt,
                        Backend::Scalar,
                        || panic!("sample failed")
                    ))
                    .is_err()
                );
                observe(Direction::Encrypt, true);
            }),
            Some(())
        );
        assert_eq!(OBSERVATION.with(Cell::get), original);
        assert!(!super::super::CALIBRATION.with(Cell::get));
    }
}
