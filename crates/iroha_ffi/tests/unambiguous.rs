//! Tests for unambiguous FFI method name generation with trait impls and inherent methods.
#![allow(unsafe_code)]

use std::mem::MaybeUninit;

use iroha_ffi::{FfiOutPtrRead, FfiReturn, FfiType, ffi_export};

/// Enum of ambiguous method sources used to validate name mangling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FfiType)]
#[repr(u8)]
pub enum Ambiguous {
    /// Inherent impl method
    Inherent,
    /// Trait `AmbiguousX` impl method
    AmbiguousX,
    /// Trait `AmbiguousY` impl method
    AmbiguousY,
    /// Placeholder value used in tests
    None,
}

/// `FfiStruct`
#[derive(Clone, Copy, FfiType)]
pub struct FfiStruct;

#[ffi_export]
impl FfiStruct {
    /// Ambiguous method
    pub fn ambiguous() -> Ambiguous {
        Ambiguous::Inherent
    }
}

/// Trait used to test name disambiguation (X).
pub trait AmbiguousX {
    /// Return an `Ambiguous` marker for this trait.
    fn ambiguous() -> Ambiguous;
}

/// Trait used to test name disambiguation (Y).
pub trait AmbiguousY {
    /// Return an `Ambiguous` marker for this trait.
    fn ambiguous() -> Ambiguous;
}

#[ffi_export]
impl AmbiguousX for FfiStruct {
    fn ambiguous() -> Ambiguous {
        Ambiguous::AmbiguousX
    }
}

#[ffi_export]
impl AmbiguousY for FfiStruct {
    fn ambiguous() -> Ambiguous {
        Ambiguous::AmbiguousY
    }
}

#[test]
fn unambiguous_method_call() {
    let mut output = MaybeUninit::new(Ambiguous::None as _);

    unsafe {
        assert_eq!(FfiReturn::Ok, FfiStruct__ambiguous(output.as_mut_ptr()));
        let inherent = Ambiguous::try_read_out(output.assume_init()).unwrap();
        assert_eq!(Ambiguous::Inherent, inherent);

        assert_eq!(
            FfiReturn::Ok,
            FfiStruct__AmbiguousX__ambiguous(output.as_mut_ptr())
        );
        let ambiguous_x = Ambiguous::try_read_out(output.assume_init()).unwrap();
        assert_eq!(Ambiguous::AmbiguousX, ambiguous_x);

        assert_eq!(
            FfiReturn::Ok,
            FfiStruct__AmbiguousY__ambiguous(output.as_mut_ptr())
        );
        let ambiguous_y = Ambiguous::try_read_out(output.assume_init()).unwrap();
        assert_eq!(Ambiguous::AmbiguousY, ambiguous_y);
    }
}
