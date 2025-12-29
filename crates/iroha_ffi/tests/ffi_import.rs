//! FFI import tests for non-opaque and robust transparent types.
#![allow(unsafe_code)]

use iroha_ffi::{LocalRef, LocalSlice, ffi, ffi_import};

ffi! {
    // NOTE: Wrapped in ffi! to test that macro expansion works for non-opaque types as well.
    /// Local transparent tuple struct used in test signatures.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[ffi_type(unsafe {robust})]
    #[repr(transparent)]
    pub struct Transparent((u32, u32));
}

/// Pass-through of a non-local reference.
#[ffi_import]
pub fn freestanding_returns_non_local(input: &u32) -> &u32 {
    unreachable!("replaced by ffi_import")
}

/// Pass-through of a tuple reference into a local reference.
#[ffi_import]
pub fn freestanding_returns_local_ref(input: &(u32, u32)) -> &(u32, u32) {
    unreachable!("replaced by ffi_import")
}

/// Copy a slice of tuples into a local slice.
#[ffi_import]
pub fn freestanding_returns_local_slice(input: &[(u32, u32)]) -> &[(u32, u32)] {
    unreachable!("replaced by ffi_import")
}

/// Copy a boxed slice.
#[ffi_import]
pub fn freestanding_returns_boxed_slice(input: Box<[u32]>) -> Box<[u32]> {
    unreachable!("replaced by ffi_import")
}

/// Convert an iterator into an exact-size iterator.
#[ffi_import]
pub fn freestanding_returns_iterator(
    input: impl IntoIterator<Item = u32>,
) -> impl ExactSizeIterator<Item = u32> {
    unreachable!("replaced by ffi_import")
}

/// Take and return an array by value.
#[ffi_import]
pub fn freestanding_take_and_return_array(input: [(u32, u32); 2]) -> impl Into<[(u32, u32); 2]> {
    unreachable!("replaced by ffi_import")
}

/// Pass-through of a local transparent reference.
#[ffi_import]
pub fn freestanding_take_and_return_local_transparent_ref(input: &Transparent) -> &Transparent {
    unreachable!("replaced by ffi_import")
}

/// Pass-through of a boxed integer.
#[ffi_import]
pub fn freestanding_take_and_return_boxed_int(input: Box<u8>) -> Box<u8> {
    unreachable!("replaced by ffi_import")
}

/// Return a `Result<(), u8>` indicating success/failure.
#[ffi_import]
pub fn freestanding_return_empty_tuple_result(flag: bool) -> Result<(), u8> {
    unreachable!("replaced by ffi_import")
}

#[test]
fn take_and_return_non_local() {
    let input = 420;
    let output: &u32 = freestanding_returns_non_local(&input);
    assert_eq!(&input, output);
}

#[test]
fn tuple_ref_is_coppied_when_returned() {
    let in_tuple = (420, 420);
    let out_tuple: LocalRef<(u32, u32)> = freestanding_returns_local_ref(&in_tuple);
    assert_eq!(in_tuple, *out_tuple);
}

#[test]
fn vec_of_tuples_is_coppied_when_returned() {
    let in_tuple = vec![(420_u32, 420_u32)];
    let out_tuple: LocalSlice<(u32, u32)> = freestanding_returns_local_slice(&in_tuple);
    assert_eq!(in_tuple, *out_tuple);
}

#[test]
fn boxed_slice_of_primitives() {
    let in_boxed_slice = vec![420_u32, 420_u32].into_boxed_slice();
    let out_boxed_slice: Box<[u32]> = freestanding_returns_boxed_slice(in_boxed_slice.clone());
    assert_eq!(in_boxed_slice, out_boxed_slice);
}

#[test]
fn return_iterator() {
    let input = vec![420_u32, 420_u32];
    let output = freestanding_returns_iterator(input.clone());
    assert_eq!(input, output);
}

#[test]
fn take_and_return_array() {
    let input = [(420, 420), (420, 420)];
    let output: [(u32, u32); 2] = freestanding_take_and_return_array(input);
    assert_eq!(input, output);
}

#[test]
fn take_and_return_transparent_local_ref() {
    let input = Transparent((420, 420));
    let output: LocalRef<Transparent> = freestanding_take_and_return_local_transparent_ref(&input);
    assert_eq!(input, *output);
}

#[test]
fn take_and_return_boxed_int() {
    let input: Box<u8> = Box::new(42u8);
    let output: Box<u8> = freestanding_take_and_return_boxed_int(input.clone());
    assert_eq!(input, output);
}

#[test]
fn return_empty_tuple_result() {
    assert!(freestanding_return_empty_tuple_result(false).is_ok());
}

mod ffi {
    use iroha_ffi::{
        FfiOutPtr, FfiReturn, FfiTuple2, FfiType,
        slice::{OutBoxedSlice, RefMutSlice, RefSlice},
    };

    iroha_ffi::def_ffi_fns! { dealloc }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn __freestanding_returns_non_local(
        input: *const u32,
        output: *mut *const u32,
    ) -> FfiReturn {
        unsafe { output.write(input) };
        FfiReturn::Ok
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn __freestanding_returns_local_ref(
        input: *const FfiTuple2<u32, u32>,
        output: *mut FfiTuple2<u32, u32>,
    ) -> FfiReturn {
        unsafe { output.write(input.read()) };
        FfiReturn::Ok
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn __freestanding_returns_local_slice(
        input: RefSlice<FfiTuple2<u32, u32>>,
        output: *mut OutBoxedSlice<FfiTuple2<u32, u32>>,
    ) -> FfiReturn {
        let input = unsafe { input.into_rust() }.map(Into::into);
        unsafe { output.write(OutBoxedSlice::from_boxed_slice(input)) };
        FfiReturn::Ok
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn __freestanding_returns_boxed_slice(
        input: RefMutSlice<u32>,
        output: *mut OutBoxedSlice<u32>,
    ) -> FfiReturn {
        let input = unsafe { input.into_rust() }.map(|slice| (&*slice).into());
        unsafe { output.write(OutBoxedSlice::from_boxed_slice(input)) };
        FfiReturn::Ok
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn __freestanding_returns_iterator(
        input: RefMutSlice<u32>,
        output: *mut OutBoxedSlice<u32>,
    ) -> FfiReturn {
        let input = unsafe { input.into_rust() }.map(|slice| (&*slice).into());
        unsafe { output.write(OutBoxedSlice::from_boxed_slice(input)) };
        FfiReturn::Ok
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn __freestanding_take_and_return_array(
        input: *mut [FfiTuple2<u32, u32>; 2],
        output: *mut [FfiTuple2<u32, u32>; 2],
    ) -> FfiReturn {
        unsafe { output.write(input.read()) };
        FfiReturn::Ok
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn __freestanding_take_and_return_local_transparent_ref(
        input: <&(u32, u32) as FfiType>::ReprC,
        output: *mut <&(u32, u32) as FfiOutPtr>::OutPtr,
    ) -> FfiReturn {
        unsafe { output.write(input.read()) };
        FfiReturn::Ok
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn __freestanding_take_and_return_boxed_int(
        input: <Box<u8> as FfiType>::ReprC,
        output: *mut <Box<u8> as FfiOutPtr>::OutPtr,
    ) -> FfiReturn {
        unsafe { output.write(input.read()) };
        FfiReturn::Ok
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn __freestanding_return_empty_tuple_result(
        input: <bool as FfiType>::ReprC,
    ) -> FfiReturn {
        if input == 1 {
            return FfiReturn::ExecutionFail;
        }

        FfiReturn::Ok
    }
}
