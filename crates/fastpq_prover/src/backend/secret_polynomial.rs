//! Fixed-extent owned polynomial storage with unconditional zeroization on drop.
//!
//! Allocate before writing private values and expose only slices thereafter, so
//! no secret-bearing reallocation occurs. Borrowed caller inputs are not erased.
//! This clears owned allocation contents on normal return and Rust unwinding; it
//! does not promise removal of copies in registers, stacks, swap, caller buffers,
//! hardware devices, or termination that bypasses destructors.

use crate::{Error, Result};
use core::ops::{Deref, DerefMut};
use zeroize::{Zeroize, Zeroizing};

/// Private fixed-extent owner; intentionally has neither Debug nor Clone.
pub(super) struct SecretPolynomial<T: Zeroize> {
    values: Zeroizing<Box<[T]>>,
}

impl<T: Zeroize + Default> SecretPolynomial<T> {
    /// Allocate and initialize under the erasure guard before any private writes.
    pub(super) fn zeroed(length: usize) -> Result<Self> {
        let bytes = length
            .checked_mul(core::mem::size_of::<T>())
            .ok_or_else(|| invalid("private polynomial byte count overflow"))?;
        if bytes > isize::MAX as usize {
            return Err(invalid(
                "private polynomial allocation exceeds addressable bytes",
            ));
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| invalid("private polynomial allocation failed"))?;
        values.resize_with(length, T::default);
        // Conversion may resize an allocation, but all elements are still the
        // field's public zero defaults. No private value is written until the
        // fixed-size box is under its erasure guard and returned to the caller.
        Ok(Self {
            values: Zeroizing::new(values.into_boxed_slice()),
        })
    }
}

impl<T: Zeroize + Default + Copy> SecretPolynomial<T> {
    /// Copy a caller-owned slice into a separately guarded fixed allocation.
    pub(super) fn from_slice(values: &[T]) -> Result<Self> {
        let mut owned = Self::zeroed(values.len())?;
        owned.copy_from_slice(values);
        Ok(owned)
    }
}

impl<T: Zeroize> Deref for SecretPolynomial<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.values
    }
}

impl<T: Zeroize> DerefMut for SecretPolynomial<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.values
    }
}

fn invalid(details: &'static str) -> Error {
    Error::InvalidTraceShape {
        details: details.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn fixed_storage_clears_all_owned_elements_on_drop_and_unwind() {
        static CLEARED: AtomicUsize = AtomicUsize::new(0);
        #[derive(Clone, Copy, Default)]
        struct Tracked(u64);
        impl Zeroize for Tracked {
            fn zeroize(&mut self) {
                assert_eq!(self.0, 71);
                self.0.zeroize();
                assert_eq!(self.0, 0);
                CLEARED.fetch_add(1, Ordering::SeqCst);
            }
        }
        let caller = [Tracked(71); 7];
        {
            let owned = SecretPolynomial::from_slice(&caller).unwrap();
            assert_eq!(owned.len(), 7);
        }
        assert_eq!(CLEARED.load(Ordering::SeqCst), 7);
        let unwind = std::panic::catch_unwind(|| {
            let _owned = SecretPolynomial::from_slice(&caller).unwrap();
            panic!("test-only unwind after private allocation");
        });
        assert!(unwind.is_err());
        assert_eq!(CLEARED.load(Ordering::SeqCst), 14);
        assert!(caller.iter().all(|value| value.0 == 71));
        assert!(SecretPolynomial::<u64>::zeroed(usize::MAX).is_err());
    }
}
