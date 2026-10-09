//! Scoped counts of the actual full-image owner, independent of temporary timing probes.

use std::cell::Cell;

thread_local! {
    static VALIDATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

pub(super) fn record() {
    VALIDATIONS.with(|value| {
        if let Some(count) = value.get() {
            value.set(Some(
                count.checked_add(1).expect("profile validation count"),
            ));
        }
    });
}

pub(in crate::managed) fn count<T>(action: impl FnOnce() -> T) -> (T, usize) {
    struct Restore(Option<usize>);
    impl Drop for Restore {
        fn drop(&mut self) {
            VALIDATIONS.with(|value| value.set(self.0));
        }
    }
    let _restore = Restore(VALIDATIONS.with(|value| value.replace(Some(0))));
    let result = action();
    (result, VALIDATIONS.with(|value| value.get().unwrap()))
}

/// Activate the existing profile counter without wrapping or moving a native result.
pub(in crate::managed) struct Counter {
    previous: Option<usize>,
}
impl Counter {
    /// Start one explicitly installed test-local observation and retain the prior counter.
    pub(in crate::managed) fn begin() -> Self {
        Self {
            previous: VALIDATIONS.with(|value| value.replace(Some(0))),
        }
    }
}
impl Drop for Counter {
    fn drop(&mut self) {
        VALIDATIONS.with(|value| value.set(self.previous));
    }
}

/// Read the existing counter without performing any profile validation.
pub(in crate::managed) fn snapshot() -> Option<usize> {
    VALIDATIONS.with(Cell::get)
}

#[test]
fn diagnostic_counter_uses_the_original_record_and_restores_its_owner() {
    assert_eq!(snapshot(), None);
    let ((), original) = count(|| {
        record();
        {
            let _counter = Counter::begin();
            assert_eq!(snapshot(), Some(0));
            record();
            record();
            assert_eq!(snapshot(), Some(2));
        }
        assert_eq!(snapshot(), Some(1));
        record();
    });
    assert_eq!(original, 2);
    assert_eq!(snapshot(), None);
}
