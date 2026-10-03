//! Nested synthetic receipt selection; work still executes the same accounting.

use std::cell::Cell;
thread_local! { static SYNTHETIC: Cell<bool> = const { Cell::new(false) }; }

pub(super) fn synthetic() -> bool {
    SYNTHETIC.with(Cell::get)
}

pub(in crate::vector) fn with_synthetic<T>(call: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            SYNTHETIC.with(|slot| slot.set(self.0));
        }
    }
    let _restore = Restore(SYNTHETIC.with(|slot| slot.replace(true)));
    call()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_synthetic_receipts_restore_after_success_and_unwind() {
        assert!(!synthetic());
        with_synthetic(|| {
            assert!(synthetic());
            with_synthetic(|| assert!(synthetic()));
            assert!(
                std::panic::catch_unwind(|| with_synthetic(|| panic!("native calibration unwind")))
                    .is_err()
            );
            assert!(synthetic());
        });
        assert!(!synthetic());
    }
}
