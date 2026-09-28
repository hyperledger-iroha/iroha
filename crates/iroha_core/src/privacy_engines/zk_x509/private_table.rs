//! Clearing ownership for private tables while fallible builders populate them.

use crate::privacy_engines::transparent_stark::GoldilocksFieldV1 as F;

/// Own a table until it is transferred into its final clearing owner.
///
/// The eraser runs on the live cells on ordinary errors and unwinding. It must
/// overwrite cells without removing elements, so their destructors still run.
pub(super) struct PrivateTableV1<T> {
    rows: Vec<T>,
    erase: fn(&mut [T]),
}

impl<T> core::fmt::Debug for PrivateTableV1<T> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("PrivateTableV1 { <private table redacted> }")
    }
}

impl<T> PrivateTableV1<T> {
    /// Guard an allocation before the first private value is written to it.
    pub(super) fn new(rows: Vec<T>, erase: fn(&mut [T])) -> Self {
        Self { rows, erase }
    }

    /// Transfer the allocation directly into another clearing owner.
    pub(super) fn into_vec(mut self) -> Vec<T> {
        core::mem::take(&mut self.rows)
    }
}

impl<T> core::ops::Deref for PrivateTableV1<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.rows
    }
}

impl<T> core::ops::DerefMut for PrivateTableV1<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.rows
    }
}

impl<T> Drop for PrivateTableV1<T> {
    fn drop(&mut self) {
        (self.erase)(&mut self.rows);
    }
}

/// Overwrite each initialized field cell without releasing its allocation.
pub(super) fn zeroize_field_rows_v1<R: AsMut<[F]>>(rows: &mut [R]) {
    #[cfg(test)]
    if inspection::enabled_v1() {
        let mut observation = inspection::ErasureObservationV1::default();
        for row in rows {
            for value in row.as_mut() {
                observation.cells += 1;
                observation.nonzero_before += usize::from(*value != F::ZERO);
                value.zeroize_v1();
                observation.nonzero_after += usize::from(*value != F::ZERO);
            }
        }
        inspection::record_v1(observation);
        return;
    }
    for row in rows {
        for value in row.as_mut() {
            value.zeroize_v1();
        }
    }
}

/// Overwrite a flat field column while preserving its initialized length.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn zeroize_fields_v1(mut fields: &mut [F]) {
    zeroize_field_rows_v1(core::slice::from_mut(&mut fields));
}

/// Overwrite private primitive words before their containing allocation is freed.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn zeroize_words_v1<T: zeroize::Zeroize + Default + PartialEq>(words: &mut [T]) {
    #[cfg(test)]
    if inspection::enabled_v1() {
        let mut observation = inspection::ErasureObservationV1::default();
        for word in words {
            observation.cells += 1;
            observation.nonzero_before += usize::from(*word != T::default());
            word.zeroize();
            observation.nonzero_after += usize::from(*word != T::default());
        }
        inspection::record_v1(observation);
        return;
    }
    for word in words {
        word.zeroize();
    }
}

/// Test observations contain counts only, captured before the allocation frees.
#[cfg(test)]
pub(super) mod inspection {
    use std::cell::RefCell;

    /// Counts observed around the real field-cell erasure operation.
    #[derive(Default)]
    pub(crate) struct ErasureObservationV1 {
        pub(crate) cells: usize,
        pub(crate) nonzero_before: usize,
        pub(crate) nonzero_after: usize,
    }

    thread_local! {
        static OBSERVATIONS: RefCell<Option<Vec<ErasureObservationV1>>> = const { RefCell::new(None) };
    }

    pub(super) fn enabled_v1() -> bool {
        OBSERVATIONS.with_borrow(Option::is_some)
    }

    pub(super) fn record_v1(observation: ErasureObservationV1) {
        OBSERVATIONS.with_borrow_mut(|observations| {
            if let Some(observations) = observations {
                observations.push(observation);
            }
        });
    }

    /// Run one fallible construction and inspect only its clearing operations.
    pub(crate) fn observe_v1<T>(operation: impl FnOnce() -> T) -> (T, Vec<ErasureObservationV1>) {
        struct ObservationScope;
        impl Drop for ObservationScope {
            fn drop(&mut self) {
                OBSERVATIONS.set(None);
            }
        }
        OBSERVATIONS.set(Some(Vec::new()));
        let _scope = ObservationScope;
        let result = operation();
        let observations = OBSERVATIONS.take().unwrap();
        (result, observations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    thread_local! {
        static CLEARED_CELLS: Cell<usize> = const { Cell::new(0) };
    }

    fn erase_and_inspect(rows: &mut [Vec<F>]) {
        let cells = rows.iter().map(Vec::len).sum::<usize>();
        zeroize_field_rows_v1(rows);
        // Inspect the actual allocation while it is still owned by the guard.
        assert!(rows.iter().flatten().all(|value| *value == F::ZERO));
        CLEARED_CELLS.set(CLEARED_CELLS.get() + cells);
    }

    #[test]
    fn private_table_clears_live_cells_on_error_and_unwind() {
        CLEARED_CELLS.set(0);
        let fail = || -> Result<(), ()> {
            let mut rows = PrivateTableV1::new(Vec::new(), erase_and_inspect);
            rows.push(vec![F(3), F(7)]);
            rows.push(vec![F(11)]);
            Err(())
        };
        assert_eq!(fail(), Err(()));
        assert_eq!(CLEARED_CELLS.get(), 3);
        assert!(
            std::panic::catch_unwind(|| {
                let _rows = PrivateTableV1::new(vec![vec![F(19); 5]], erase_and_inspect);
                panic!("exercise private table unwinding");
            })
            .is_err()
        );
        assert_eq!(CLEARED_CELLS.get(), 8);
    }

    #[test]
    fn private_table_transfer_preserves_values_until_next_owner_drops() {
        CLEARED_CELLS.set(0);
        let rows = PrivateTableV1::new(vec![vec![F(23); 4]], erase_and_inspect);
        let allocation = rows.as_ptr();
        let transferred = rows.into_vec();
        assert_eq!(transferred.as_ptr(), allocation);
        assert_eq!(transferred, vec![vec![F(23); 4]]);
        assert_eq!(CLEARED_CELLS.get(), 0);
        drop(PrivateTableV1::new(transferred, erase_and_inspect));
        assert_eq!(CLEARED_CELLS.get(), 4);
    }

    #[test]
    fn field_array_rows_are_cleared_in_place() {
        let mut rows = [[F(5), F(8)]; 3];
        zeroize_field_rows_v1(&mut rows);
        assert_eq!(rows, [[F::ZERO; 2]; 3]);
    }
    #[test]
    fn flat_fields_and_primitive_words_are_observed_before_release() {
        let mut fields = [F(7), F(11)];
        let mut bytes = [19u8; 3];
        let mut limbs = [23u16; 5];
        let (_, observations) = inspection::observe_v1(|| {
            zeroize_fields_v1(&mut fields);
            zeroize_words_v1(&mut bytes);
            zeroize_words_v1(&mut limbs);
        });
        assert_eq!(fields, [F::ZERO; 2]);
        assert_eq!(bytes, [0; 3]);
        assert_eq!(limbs, [0; 5]);
        assert_eq!(
            observations.iter().map(|item| item.cells).sum::<usize>(),
            10
        );
        assert_eq!(
            observations
                .iter()
                .map(|item| item.nonzero_before)
                .sum::<usize>(),
            10
        );
        assert!(observations.iter().all(|item| item.nonzero_after == 0));
        // Measurements outside an observation scope use only the real eraser.
        assert!(!inspection::enabled_v1());
    }
}
