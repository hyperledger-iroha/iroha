//! Heap owner for fixed-size secret arrays of the canonical V1 contract.
//!
//! The array lives in one heap allocation for its whole life. Moving the owner
//! moves a pointer, so a move never leaves the secret behind in a stack frame.
//! The allocation is cleared when the owner drops: on success, on an error
//! return and during unwinding. Scalars the compiler copies into registers or
//! temporaries while computing on the array are not covered.

use std::ops::{Deref, DerefMut};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;

/// One heap allocation of `N` secret scalars, cleared on drop.
pub(super) struct ClearingArray<T, const N: usize>(Box<[T; N]>)
where
    T: Zeroize + Copy + Default + ConstantTimeEq;

impl<T, const N: usize> ClearingArray<T, N>
where
    T: Zeroize + Copy + Default + ConstantTimeEq,
{
    /// Allocate the owner with every cell at its default, which is zero for the
    /// integer cells this owner holds. The caller fills it in place.
    pub(super) fn zeroed() -> Self {
        Self(Box::new([T::default(); N]))
    }

    /// Copy a borrowed array into a new allocation.
    pub(super) fn copy_of(value: &[T; N]) -> Self {
        let mut owner = Self::zeroed();
        owner.0.copy_from_slice(value);
        owner
    }

    /// Return whether every cell is at its zero default, without an early exit.
    pub(super) fn is_zero(&self) -> bool {
        self.0.as_slice().ct_eq([T::default(); N].as_slice()).into()
    }
}

impl<T, const N: usize> Deref for ClearingArray<T, N>
where
    T: Zeroize + Copy + Default + ConstantTimeEq,
{
    type Target = [T; N];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, const N: usize> DerefMut for ClearingArray<T, N>
where
    T: Zeroize + Copy + Default + ConstantTimeEq,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T, const N: usize> Clone for ClearingArray<T, N>
where
    T: Zeroize + Copy + Default + ConstantTimeEq,
{
    fn clone(&self) -> Self {
        Self::copy_of(&self.0)
    }
}

impl<T, const N: usize> PartialEq for ClearingArray<T, N>
where
    T: Zeroize + Copy + Default + ConstantTimeEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.0.as_slice().ct_eq(other.0.as_slice()).into()
    }
}

impl<T, const N: usize> Eq for ClearingArray<T, N> where T: Zeroize + Copy + Default + ConstantTimeEq
{}

impl<T, const N: usize> Drop for ClearingArray<T, N>
where
    T: Zeroize + Copy + Default + ConstantTimeEq,
{
    fn drop(&mut self) {
        <[T; N] as Zeroize>::zeroize(&mut self.0);
        #[cfg(test)]
        observe_clear(self.is_zero(), N);
    }
}

#[cfg(test)]
thread_local! {
    static CLEARED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Record that an owner dropped with every one of its `cells` cleared.
#[cfg(test)]
fn observe_clear(cleared: bool, cells: usize) {
    assert!(cleared, "a clearing owner dropped with a nonzero cell");
    CLEARED.with(|count| count.set(count.get() + cells));
}

/// Number of cells this thread has observed cleared at drop.
#[cfg(test)]
pub(super) fn cleared_cells() -> usize {
    CLEARED.with(std::cell::Cell::get)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled() -> ClearingArray<u16, 8> {
        ClearingArray::copy_of(&[257, 1, 2, 3, 4, 5, 6, u16::MAX])
    }

    #[test]
    fn owner_is_one_allocation_that_moves_by_pointer() {
        let owner = filled();
        let address = owner.as_ptr();
        // Moving the owner, into a function and into a box, keeps the cells in place.
        let moved = std::convert::identity(owner);
        assert_eq!(moved.as_ptr(), address);
        let boxed = Box::new(moved);
        assert_eq!(boxed.as_ptr(), address);
        assert_eq!(**boxed, [257, 1, 2, 3, 4, 5, 6, u16::MAX]);
    }

    #[test]
    fn zeroed_copy_clone_and_equality_cover_every_cell() {
        let zero = ClearingArray::<u8, 32>::zeroed();
        assert_eq!(*zero, [0; 32]);
        assert!(zero.is_zero());
        let owner = filled();
        assert!(!owner.is_zero());
        let clone = owner.clone();
        assert_ne!(clone.as_ptr(), owner.as_ptr());
        assert!(clone == owner);
        // Equality covers the first and the last cell.
        for cell in [0, 7] {
            let mut changed = owner.clone();
            changed[cell] ^= 1;
            assert!(changed != owner);
        }
        let mut last = ClearingArray::<u8, 32>::zeroed();
        last[31] = 1;
        assert!(!last.is_zero());
    }

    #[test]
    fn allocation_clears_on_drop_error_and_unwind() {
        let before = cleared_cells();
        drop(filled());
        assert_eq!(cleared_cells() - before, 8);
        let failing = || -> Result<(), ()> {
            let _owner = filled();
            Err(())
        };
        assert!(failing().is_err());
        assert_eq!(cleared_cells() - before, 16);
        assert!(
            std::panic::catch_unwind(|| {
                let _owner = filled();
                panic!("clearing owner unwind control");
            })
            .is_err()
        );
        assert_eq!(cleared_cells() - before, 24);
    }
}
