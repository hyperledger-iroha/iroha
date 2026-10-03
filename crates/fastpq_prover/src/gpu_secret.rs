//! Clearing fixed-extent host staging for accelerator-owned private words.
//!
//! This owner clears its allocation on normal return and unwinding. Borrowed
//! caller storage and device allocations have separate owners. It intentionally
//! exposes no resizing operations after private values have been written.

use std::{
    collections::TryReserveError,
    ops::{Deref, DerefMut},
};
use zeroize::Zeroize as _;

pub struct SecretWords(Vec<u64>);

impl SecretWords {
    pub fn zeroed(length: usize) -> Result<Self, TryReserveError> {
        let mut words = Vec::new();
        words.try_reserve_exact(length)?;
        words.resize(length, 0);
        Ok(Self(words))
    }

    pub fn copy_from(values: &[u64]) -> Result<Self, TryReserveError> {
        let mut words = Self::zeroed(values.len())?;
        words.copy_from_slice(values);
        Ok(words)
    }

    /// Restore the original allocation and take ownership of the replaced one.
    #[cfg(any(test, target_os = "macos"))]
    pub fn swap_with(&mut self, destination: &mut Vec<u64>) {
        std::mem::swap(&mut self.0, destination);
    }

    /// Transfer completed output to the caller's existing ownership boundary.
    pub fn into_vec(mut self) -> Vec<u64> {
        std::mem::take(&mut self.0)
    }
}

/// Allocate the outer output list before transferring any private allocation.
pub fn release_columns(columns: Vec<SecretWords>) -> Result<Vec<Vec<u64>>, TryReserveError> {
    let mut output = Vec::new();
    output.try_reserve_exact(columns.len())?;
    for column in columns {
        output.push(column.into_vec());
    }
    Ok(output)
}

impl Deref for SecretWords {
    type Target = [u64];
    fn deref(&self) -> &[u64] {
        &self.0
    }
}

impl DerefMut for SecretWords {
    fn deref_mut(&mut self) -> &mut [u64] {
        &mut self.0
    }
}

impl Drop for SecretWords {
    fn drop(&mut self) {
        self.0.as_mut_slice().zeroize();
        #[cfg(test)]
        observe_cleared_words(&self.0);
    }
}

#[cfg(test)]
pub fn observe_cleared_words(words: &[u64]) {
    OBSERVATION.with(|observation| {
        if let Some((cleared, uncleared)) = observation.get() {
            let clean = words.iter().filter(|&&word| word == 0).count();
            observation.set(Some((cleared + clean, uncleared + words.len() - clean)));
        }
    });
}

#[cfg(test)]
std::thread_local! {
    static OBSERVATION: std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
// This guard owns a thread-local observation and cannot move to another thread.
pub struct ErasureObservation {
    thread: std::thread::ThreadId,
    _local: std::marker::PhantomData<std::rc::Rc<()>>,
}
#[cfg(test)]
impl ErasureObservation {
    pub fn begin() -> Self {
        OBSERVATION.with(|observation| {
            // A refused nested scope must preserve the original observation.
            assert_eq!(observation.get(), None);
            observation.set(Some((0, 0)));
        });
        Self {
            thread: std::thread::current().id(),
            _local: std::marker::PhantomData,
        }
    }
    pub fn counts(&self) -> (usize, usize) {
        assert_eq!(self.thread, std::thread::current().id());
        OBSERVATION.with(|observation| observation.get().unwrap())
    }
}
#[cfg(test)]
impl Drop for ErasureObservation {
    fn drop(&mut self) {
        OBSERVATION.with(|observation| observation.set(None));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refused_nested_observation_preserves_original_counts_and_drop_reopens_scope() {
        let observed = ErasureObservation::begin();
        drop(SecretWords::copy_from(&[11, 13, 17, 19]).unwrap());
        assert_eq!(observed.counts(), (4, 0));
        let nested = std::panic::catch_unwind(ErasureObservation::begin);
        assert!(nested.is_err());
        assert_eq!(observed.counts(), (4, 0));
        drop(SecretWords::copy_from(&[23, 29]).unwrap());
        assert_eq!(observed.counts(), (6, 0));
        drop(observed);
        let next = ErasureObservation::begin();
        assert_eq!(next.counts(), (0, 0));
        drop(SecretWords::copy_from(&[31]).unwrap());
        assert_eq!(next.counts(), (1, 0));
    }

    #[test]
    fn staged_words_clear_real_cells_on_success_partial_error_and_unwind() {
        let observed = ErasureObservation::begin();
        let source = [11, 29, 47, u64::MAX];
        drop(SecretWords::copy_from(&source).unwrap());
        assert_eq!(observed.counts(), (4, 0));
        let allocation_failure = (|| {
            let _earlier_column = SecretWords::copy_from(&source)?;
            SecretWords::zeroed(usize::MAX)
        })();
        assert!(allocation_failure.is_err());
        assert_eq!(observed.counts(), (8, 0));
        let unwind = std::panic::catch_unwind(|| {
            let _inflight = SecretWords::copy_from(&source).unwrap();
            panic!("private staging unwind fixture");
        });
        assert!(unwind.is_err());
        assert_eq!(observed.counts(), (12, 0));
        assert_eq!(source, [11, 29, 47, u64::MAX]);
    }

    #[test]
    fn rollback_preserves_original_allocation_and_clears_replaced_cells() {
        let observed = ErasureObservation::begin();
        let mut original = SecretWords::copy_from(&[17, 19, 23]).unwrap();
        let original_address = original.as_ptr();
        let mut destination = vec![71, 73, 79];
        original.swap_with(&mut destination);
        assert_eq!(destination, [17, 19, 23]);
        assert_eq!(destination.as_ptr(), original_address);
        assert_eq!(&*original, [71, 73, 79]);
        drop(original);
        assert_eq!(observed.counts(), (3, 0));
    }

    #[test]
    fn complete_column_list_transfer_preserves_order_and_allocations() {
        let observed = ErasureObservation::begin();
        let columns = vec![
            SecretWords::copy_from(&[11, 13]).unwrap(),
            SecretWords::copy_from(&[17, 19]).unwrap(),
        ];
        let addresses = columns
            .iter()
            .map(|column| column.as_ptr())
            .collect::<Vec<_>>();
        let result = release_columns(columns).unwrap();
        assert_eq!(result, [vec![11, 13], vec![17, 19]]);
        assert_eq!(
            result.iter().map(Vec::as_ptr).collect::<Vec<_>>(),
            addresses
        );
        assert_eq!(observed.counts(), (0, 0));
    }

    #[test]
    fn completed_output_transfers_without_copy_or_premature_erasure() {
        let observed = ErasureObservation::begin();
        let words = SecretWords::copy_from(&[31, 37]).unwrap();
        let address = words.as_ptr();
        let output = words.into_vec();
        assert_eq!(output, [31, 37]);
        assert_eq!(output.as_ptr(), address);
        assert_eq!(observed.counts(), (0, 0));
    }
}
