//! Logic related to the conversion of slices to and from FFI-compatible representation
#![allow(unsafe_op_in_unsafe_fn)]

use core::{ptr::NonNull, slice};
use std::{boxed::Box, vec::Vec};

use crate::ReprC;

/// Immutable slice `&[C]` with a defined C ABI layout. Consists of a data pointer and a length.
///
/// If the data pointer is set to `null`, the struct represents `Option<&[C]>`.
#[repr(C)]
#[derive(Debug)]
pub struct RefSlice<C>(*const C, usize);

/// Mutable slice `&mut [C]` with a defined C ABI layout. Consists of a data pointer and a length.
///
/// If the data pointer is set to `null`, the struct represents `Option<&mut [C]>`.
#[repr(C)]
#[derive(Debug)]
pub struct RefMutSlice<C>(*mut C, usize);

/// Owned slice `Box<[C]>` with a defined C ABI layout. Consists of a data pointer and a length.
///
/// Used in place of a function out-pointer to transfer ownership of the slice to the caller.
/// If the data pointer is set to `null`, the struct represents `Option<Box<[C]>>`.
#[repr(C)]
#[derive(Debug)]
pub struct OutBoxedSlice<C>(*mut C, usize);

impl<C> Copy for RefSlice<C> {}
impl<C> Clone for RefSlice<C> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<C> Copy for RefMutSlice<C> {}
impl<C> Clone for RefMutSlice<C> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<C> Copy for OutBoxedSlice<C> {}
impl<C> Clone for OutBoxedSlice<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> RefSlice<C> {
    /// Set the slice's data pointer to null
    pub const fn null() -> Self {
        Self(core::ptr::null(), 0)
    }

    /// Create a slice from a data pointer and a length.
    pub fn from_raw_parts(ptr: *const C, len: usize) -> Self {
        Self(ptr, len)
    }

    /// Returns a raw pointer to the slice's buffer.
    pub fn as_ptr(&self) -> *const C {
        self.0
    }

    /// Returns `true` if the slice contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of elements in the slice.
    pub fn len(&self) -> usize {
        self.1
    }

    /// Create [`Self`] from shared slice
    pub const fn from_slice(source: Option<&[C]>) -> Self {
        if let Some(slice) = source {
            Self(slice.as_ptr(), slice.len())
        } else {
            Self(core::ptr::null(), 0)
        }
    }

    /// Convert [`Self`] into a shared slice. Return `None` if data pointer is null.
    /// Unlike [`core::slice::from_raw_parts`], data pointer is allowed to be null.
    ///
    /// # Safety
    ///
    /// Check [`core::slice::from_raw_parts`]
    pub unsafe fn into_rust<'slice>(self) -> Option<&'slice [C]> {
        if self.0.is_null() {
            return None;
        }

        Some(slice::from_raw_parts(self.0, self.1))
    }
}
impl<C> RefMutSlice<C> {
    /// Set the slice's data pointer to null
    pub const fn null_mut() -> Self {
        Self(core::ptr::null_mut(), 0)
    }

    /// Create a slice from a data pointer and a length.
    pub fn from_raw_parts_mut(ptr: *mut C, len: usize) -> Self {
        Self(ptr, len)
    }

    /// Returns a raw pointer to the slice's buffer.
    pub fn as_mut_ptr(&self) -> *mut C {
        self.0
    }

    /// Returns `true` if the slice contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns the number of elements in the slice.
    pub fn len(&self) -> usize {
        self.1
    }

    /// Create [`Self`] from mutable slice
    pub fn from_slice(source: Option<&mut [C]>) -> Self {
        source.map_or_else(
            || Self(core::ptr::null_mut(), 0),
            |slice| Self(slice.as_mut_ptr(), slice.len()),
        )
    }

    /// Convert [`Self`] into a mutable slice. Return `None` if data pointer is null.
    /// Unlike [`core::slice::from_raw_parts_mut`], data pointer is allowed to be null.
    ///
    /// # Safety
    ///
    /// Check [`core::slice::from_raw_parts_mut`]
    pub unsafe fn into_rust<'slice>(self) -> Option<&'slice mut [C]> {
        if self.0.is_null() {
            return None;
        }

        Some(slice::from_raw_parts_mut(self.0, self.1))
    }
}
impl<C: ReprC> OutBoxedSlice<C> {
    /// Create a slice from a data pointer and a length.
    pub fn from_raw_parts(ptr: *mut C, len: usize) -> Self {
        Self(ptr, len)
    }

    /// Return a raw pointer to the slice's buffer.
    pub fn as_mut_ptr(&self) -> *mut C {
        self.0
    }

    /// Return `true` if the slice contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return the number of elements in the slice.
    pub fn len(&self) -> usize {
        self.1
    }

    /// Create [`Self`] from a [`Box<[T]>`]
    pub fn from_boxed_slice(source: Option<Box<[C]>>) -> Self {
        source.map_or_else(
            || Self(core::ptr::null_mut(), 0),
            |boxed_slice| {
                let mut boxed_slice = core::mem::ManuallyDrop::new(boxed_slice);
                Self(boxed_slice.as_mut_ptr(), boxed_slice.len())
            },
        )
    }

    /// Create a `Vec<T>` directly from the raw components of another vector.
    /// Unlike [`Vec::from_raw_parts`], data pointer is allowed to be null.
    ///
    /// # Safety
    ///
    /// Check [`Vec::from_raw_parts`]
    pub unsafe fn into_rust(self) -> Option<Vec<C>> {
        unsafe { OwnedLinearSlice::from_out_boxed(self) }.into_vec()
    }
}

// SAFETY: Robust type with a defined C ABI
unsafe impl<T: ReprC> ReprC for RefSlice<T> {}
// SAFETY: Robust type with a defined C ABI
unsafe impl<T: ReprC> ReprC for RefMutSlice<T> {}
// SAFETY: Robust type with a defined C ABI
unsafe impl<T: ReprC> ReprC for OutBoxedSlice<T> {}

/// Linear view over a pointer/length pair originating from a [`RefMutSlice`] or [`RefSlice`].
///
/// The constructor is `unsafe` because the caller must uphold the pointer validity invariant.
/// Once created, all accessors are safe and keep the pointer bookkeeping in a single place.
#[derive(Debug, Clone)]
pub struct LinearSlice<T> {
    ptr: Option<NonNull<T>>,
    len: usize,
    mutable: bool,
}

impl<T> LinearSlice<T> {
    fn new(ptr: Option<NonNull<T>>, len: usize, mutable: bool) -> Self {
        Self { ptr, len, mutable }
    }

    /// # Safety
    ///
    /// `ptr` must be either null or point to `len` consecutive, properly aligned values of `T`.
    pub unsafe fn from_raw_parts(ptr: *mut T, len: usize) -> Self {
        Self::new(NonNull::new(ptr), len, true)
    }

    /// # Safety
    ///
    /// `ptr` must be either null or point to `len` consecutive, properly aligned values of `T`.
    pub unsafe fn from_const_raw_parts(ptr: *const T, len: usize) -> Self {
        Self::new(NonNull::new(ptr.cast_mut()), len, false)
    }

    /// Construct a linear view from [`RefSlice`].
    ///
    /// # Safety
    ///
    /// Matches [`Self::from_const_raw_parts`]; the input must be valid for reads.
    pub unsafe fn from_ref(slice: RefSlice<T>) -> Self {
        Self::from_const_raw_parts(slice.as_ptr(), slice.len())
    }

    /// Construct a linear view from [`RefMutSlice`].
    ///
    /// # Safety
    ///
    /// Matches [`Self::from_raw_parts`]; the input must be valid for mutable access.
    pub unsafe fn from_ref_mut(slice: RefMutSlice<T>) -> Self {
        Self::from_raw_parts(slice.as_mut_ptr(), slice.len())
    }

    /// Returns the number of elements if the pointer is non-null.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ptr.map_or(0, |_| self.len)
    }

    /// Returns `true` when the slice is empty or the pointer is null.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Exposes a shared slice view when the pointer is present.
    #[must_use]
    pub fn try_borrow(&self) -> Option<&[T]> {
        self.ptr
            .map(|ptr| unsafe { slice::from_raw_parts(ptr.as_ptr().cast_const(), self.len) })
    }

    /// Exposes a mutable slice view when the pointer is present.
    #[must_use]
    pub fn try_borrow_mut(&mut self) -> Option<&mut [T]> {
        if !self.mutable {
            return None;
        }

        self.ptr
            .map(|ptr| unsafe { slice::from_raw_parts_mut(ptr.as_ptr(), self.len) })
    }

    /// Reconstruct a [`RefSlice`] for FFI calls.
    #[must_use]
    pub fn as_ref_slice(&self) -> RefSlice<T> {
        self.ptr.map_or_else(RefSlice::null, |ptr| {
            RefSlice::from_raw_parts(ptr.as_ptr().cast_const(), self.len)
        })
    }

    /// Reconstruct a [`RefMutSlice`] for FFI calls.
    #[must_use]
    pub fn as_ref_mut_slice(&self) -> RefMutSlice<T> {
        debug_assert!(
            self.mutable,
            "Attempted to build mutable slice from shared data"
        );
        self.ptr.map_or_else(RefMutSlice::null_mut, |ptr| {
            RefMutSlice::from_raw_parts_mut(ptr.as_ptr(), self.len)
        })
    }
}

/// RAII wrapper over an owned pointer/length pair produced by [`OutBoxedSlice`].
///
/// Keeps allocation and deallocation logic in one place and offers safe high-level operations.
#[derive(Debug, Clone)]
pub struct OwnedLinearSlice<T> {
    inner: LinearSlice<T>,
}

impl<T: ReprC> OwnedLinearSlice<T> {
    /// # Safety
    ///
    /// Follows the safety contract of [`LinearSlice::from_raw_parts`].
    pub unsafe fn from_raw_parts(ptr: *mut T, len: usize) -> Self {
        Self {
            inner: LinearSlice::new(NonNull::new(ptr), len, true),
        }
    }

    /// # Safety
    ///
    /// Input must come from a valid [`OutBoxedSlice`].
    pub unsafe fn from_out_boxed(slice: OutBoxedSlice<T>) -> Self {
        Self::from_raw_parts(slice.as_mut_ptr(), slice.len())
    }

    /// Returns the number of elements if the pointer is present.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the slice does not contain elements.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Shared view for inspection.
    #[must_use]
    pub fn try_borrow(&self) -> Option<&[T]> {
        self.inner.try_borrow()
    }

    /// Mutable view for in-place mutations.
    #[must_use]
    pub fn try_borrow_mut(&mut self) -> Option<&mut [T]> {
        self.inner.try_borrow_mut()
    }

    /// Rebuild a [`RefSlice`] for downstream FFI code.
    #[must_use]
    pub fn as_ref_slice(&self) -> RefSlice<T> {
        self.inner.as_ref_slice()
    }

    /// Rebuild a [`RefMutSlice`] for downstream FFI code.
    #[must_use]
    pub fn as_ref_mut_slice(&self) -> RefMutSlice<T> {
        self.inner.as_ref_mut_slice()
    }

    /// Consume the wrapper and rebuild a boxed slice.
    #[must_use]
    pub fn into_box(self) -> Option<Box<[T]>> {
        let Self { mut inner } = self;
        inner.ptr.take().map(|ptr| unsafe {
            Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr.as_ptr(), inner.len))
        })
    }

    /// Consume the wrapper and rebuild a vector.
    #[must_use]
    pub fn into_vec(self) -> Option<Vec<T>> {
        self.into_box().map(Vec::from)
    }

    /// Release the allocation using the host `dealloc` helper.
    #[must_use]
    pub fn deallocate(mut self) -> bool {
        let Some(ptr) = self.inner.ptr.take() else {
            return true;
        };

        unsafe {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
                ptr.as_ptr(),
                self.inner.len,
            )));
        }

        true
    }
}

impl<T: ReprC> From<OwnedLinearSlice<T>> for LinearSlice<T> {
    fn from(value: OwnedLinearSlice<T>) -> Self {
        value.inner
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

    #[test]
    fn linear_slice_mut_allows_updates() {
        let mut data = vec![1u32, 2, 3].into_boxed_slice();
        let repr = RefMutSlice::from_slice(Some(&mut data));
        let mut linear = unsafe { LinearSlice::from_ref_mut(repr) };

        let slice = linear.try_borrow_mut().expect("mutable slice available");
        slice[1] = 9;

        assert_eq!(&*data, &[1, 9, 3]);
    }

    #[test]
    fn linear_slice_shared_rejects_mutation() {
        let data = vec![5u8, 6, 7].into_boxed_slice();
        let repr = RefSlice::from_slice(Some(&data));
        let mut linear = unsafe { LinearSlice::from_ref(repr) };

        assert!(linear.try_borrow().is_some());
        assert!(linear.try_borrow_mut().is_none());
    }

    #[test]
    fn owned_linear_slice_recovers_vec() {
        let boxed = vec![10u32, 20, 30].into_boxed_slice();
        let out = OutBoxedSlice::from_boxed_slice(Some(boxed));
        let vec = unsafe { OwnedLinearSlice::from_out_boxed(out) }
            .into_vec()
            .expect("vector restored");

        assert_eq!(vec, [10, 20, 30]);
    }

    #[test]
    fn linear_slice_handles_null_pointer() {
        let mut shared = unsafe { LinearSlice::<u32>::from_ref(RefSlice::null()) };
        assert!(shared.is_empty());
        assert!(shared.try_borrow().is_none());
        assert!(shared.try_borrow_mut().is_none());
        assert_eq!(shared.as_ref_slice().len(), 0);

        let mut mutable = unsafe { LinearSlice::<u32>::from_ref_mut(RefMutSlice::null_mut()) };
        assert!(mutable.is_empty());
        assert!(mutable.try_borrow().is_none());
        assert!(mutable.try_borrow_mut().is_none());
        assert_eq!(mutable.as_ref_mut_slice().len(), 0);
    }

    #[test]
    fn owned_linear_slice_deallocates_zero_sized() {
        let boxed: Box<[u32]> = Vec::new().into_boxed_slice();
        let slice = OutBoxedSlice::from_boxed_slice(Some(boxed));
        let owned = unsafe { OwnedLinearSlice::from_out_boxed(slice) };

        assert!(owned.deallocate());
    }

    #[test]
    fn owned_linear_slice_deallocates_null_pointer() {
        let owned = unsafe {
            OwnedLinearSlice::<u32>::from_out_boxed(OutBoxedSlice::from_boxed_slice(None))
        };

        assert!(owned.deallocate());
    }
}
