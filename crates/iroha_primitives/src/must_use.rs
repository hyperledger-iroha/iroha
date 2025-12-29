//! Utility to mark returned values as [`must_use`](https://doc.rust-lang.org/reference/attributes/diagnostics.html#the-must_use-attribute).
//!
//! Wrapping a value in [`MustUse`] triggers a compiler warning if the caller
//! drops it without handling, preventing accidental omission of important
//! results. This is particularly helpful for functions returning [`Result`]
//! that might otherwise be ignored in tests or prototypes.

use core::borrow::{Borrow, BorrowMut};

use derive_more::{AsMut, AsRef, Constructor, Deref, Display};

/// Wrapper type that propagates the `#[must_use]` attribute to any inner value.
///
/// This is most commonly used with [`Result`] to ensure that error values are
/// not dropped silently.
///
/// # Example
/// ```
/// use iroha_primitives::must_use::MustUse;
///
/// fn is_odd(num: i32) -> Result<MustUse<bool>, String> {
///     if num < 0 {
///         return Err(String::from("Number must be positive"));
///     }
///
///     if num % 2 == 0 {
///         Ok(MustUse::new(true))
///     } else {
///         Ok(MustUse::new(false))
///     }
/// }
///
/// if *is_odd(2).unwrap() {
///     println!("2 is odd");
/// }
///
/// // Will produce a warning, because `#[warn(unused_must_use)]` is on by default
/// // is_odd(3).unwrap();
/// ```
#[derive(
    Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Constructor, AsRef, AsMut, Deref,
)]
#[repr(transparent)]
#[must_use]
pub struct MustUse<T>(pub T);

impl<T> MustUse<T> {
    /// Consumes the wrapper, returning the inner value.
    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> From<T> for MustUse<T> {
    fn from(source: T) -> Self {
        MustUse(source)
    }
}

impl<T> Borrow<T> for MustUse<T> {
    #[inline]
    fn borrow(&self) -> &T {
        &self.0
    }
}

impl<T> BorrowMut<T> for MustUse<T> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
