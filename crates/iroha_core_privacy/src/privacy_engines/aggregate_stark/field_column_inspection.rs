//! Counts around the unchanged final-column zeroizer, without copying private cells.

use std::cell::RefCell;

use super::F;

/// Initialized-cell counts at one actual final-column destruction boundary.
#[derive(Debug)]
pub(crate) struct Erasure {
    /// Number of initialized cells owned by the original column.
    pub(crate) cells: usize,
    /// Nonzero initialized cells immediately before its canonical eraser.
    pub(crate) nonzero_before: usize,
    /// Nonzero initialized cells after erasure and before deallocation.
    pub(crate) nonzero_after: usize,
}

thread_local! {
    static OBSERVATIONS: RefCell<Option<Vec<Erasure>>> = const { RefCell::new(None) };
}

/// Inspect only when this thread has explicitly enabled the bounded test scope.
pub(super) fn before_v1(values: &[F]) -> Option<usize> {
    OBSERVATIONS.with_borrow(|observations| {
        observations
            .as_ref()
            .map(|_| values.iter().filter(|value| **value != F::ZERO).count())
    })
}

/// Record counts while the same erased original allocation is still live.
pub(super) fn after_v1(before: Option<usize>, values: &[F]) {
    if let Some(nonzero_before) = before {
        OBSERVATIONS.with_borrow_mut(|observations| {
            observations
                .as_mut()
                .expect("active erasure scope")
                .push(Erasure {
                    cells: values.len(),
                    nonzero_before,
                    nonzero_after: values.iter().filter(|value| **value != F::ZERO).count(),
                });
        });
    }
}

/// Observe final-column drops; preserve no field values or released pointers.
pub(crate) fn observe_v1<T>(operation: impl FnOnce() -> T) -> (T, Vec<Erasure>) {
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            OBSERVATIONS.set(None);
        }
    }
    OBSERVATIONS.with_borrow(|observations| assert!(observations.is_none()));
    // The routing controls destroy at most eight columns within one scope.
    OBSERVATIONS.set(Some(Vec::with_capacity(8)));
    let _scope = Scope;
    let result = operation();
    (result, OBSERVATIONS.take().expect("active erasure scope"))
}
