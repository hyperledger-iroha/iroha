//! Existing native host facilities are absent from the separately selected carrier profile.
//!
//! This owner never allocates, initializes on access or substitutes an empty native service.
//! VM entry points reject unavailable capabilities with their ordinary typed permission error.
//! The assertions here catch programming errors in internal configuration helpers after that
//! dispatch boundary; they cannot turn an absent service into native execution authority.

use core::ops::{Deref, DerefMut};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NativeFacilities {
    Available,
    UnavailableForCarrier,
}

pub(super) struct NativeOnly<T>(Option<T>);

impl<T> NativeOnly<T> {
    pub(super) const fn present(value: T) -> Self {
        Self(Some(value))
    }

    pub(super) const fn absent() -> Self {
        Self(None)
    }

    pub(super) fn construct(facilities: NativeFacilities, build: impl FnOnce() -> T) -> Self {
        match facilities {
            NativeFacilities::Available => Self::present(build()),
            NativeFacilities::UnavailableForCarrier => Self::absent(),
        }
    }

    pub(super) const fn is_present(&self) -> bool {
        self.0.is_some()
    }

    pub(super) fn require_mut(&mut self) -> &mut T {
        self.0
            .as_mut()
            .expect("native facility is unavailable to the carrier host")
    }
}

impl<T> Deref for NativeOnly<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0
            .as_ref()
            .expect("native facility is unavailable to the carrier host")
    }
}

impl<T> DerefMut for NativeOnly<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.require_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    #[test]
    fn absent_native_facilities_never_construct_or_initialize_on_access() {
        let called = Cell::new(false);
        let mut absent = NativeOnly::construct(NativeFacilities::UnavailableForCarrier, || {
            called.set(true);
            7_u64
        });
        assert!(!called.get());
        assert!(!absent.is_present());
        assert!(std::panic::catch_unwind(|| **&absent).is_err());
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                *absent.require_mut() = 11;
            }))
            .is_err()
        );
        assert!(!absent.is_present());
        assert!(!called.get());
    }

    #[test]
    fn native_facilities_construct_once_and_keep_the_original_payload() {
        let calls = Cell::new(0);
        let mut present = NativeOnly::construct(NativeFacilities::Available, || {
            calls.set(calls.get() + 1);
            7_u64
        });
        assert_eq!(calls.get(), 1);
        assert!(present.is_present());
        assert_eq!(*present, 7);
        *present.require_mut() = 11;
        assert_eq!(*present, 11);
        assert_eq!(calls.get(), 1);
    }
}
