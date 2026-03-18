//! FFI import tests for shared functions and opaque types.
#![allow(unsafe_code)]

use iroha_ffi::{ffi, ffi_import};

iroha_ffi::handles! {FfiStruct<bool>}
iroha_ffi::decl_ffi_fns! {Drop, Clone, Eq, Ord}

mod types {
    //! FFI structs used by the shared-function import tests.
    use super::*;
    ffi! {
        /// Struct without a repr attribute is opaque by default
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
        // NOTE: Replaced by the `ffi` macro
        pub struct FfiStruct<T>;
    }

    /// Public shim to obtain a `RefFfiStruct` without calling a private method across the module boundary.
    pub fn ref_of<T>(v: &super::FfiStruct<T>) -> super::RefFfiStruct<'_, T>
    where
        super::FfiStruct<T>: iroha_ffi::Handle,
    {
        v.as_ref()
    }
}

pub use types::*;

#[ffi_import]
impl FfiStruct<bool> {
    /// New
    pub fn new(name: String) -> Self {
        unreachable!("replaced by ffi_import")
    }
}

#[test]
#[allow(clippy::nonminimal_bool)]
fn import_shared_fns() {
    let ffi_struct = FfiStruct::new("ipso facto".to_string());
    let ref_ffi_struct: RefFfiStruct<_> = ref_of(&ffi_struct);
    let cloned_ffi_struct: FfiStruct<_> = Clone::clone(&ref_ffi_struct);

    assert!(*ref_ffi_struct == *ref_of(&cloned_ffi_struct));
    assert!(!(*ref_ffi_struct < *ref_of(&cloned_ffi_struct)));
}

mod ffi {
    use iroha_ffi::{FfiReturn, FfiType, def_ffi_fns, slice::RefMutSlice};

    iroha_ffi::handles! {ExternFfiStruct}

    def_ffi_fns! {
        Drop: {ExternFfiStruct},
        Clone: {ExternFfiStruct},
        Eq: {ExternFfiStruct},
        Ord: {ExternFfiStruct}
    }

    iroha_ffi::def_ffi_fns! { dealloc }

    /// Structure that `Value` points to
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, FfiType)]
    #[ffi_type(opaque)]
    #[repr(C)]
    pub struct ExternFfiStruct(pub String);

    #[unsafe(no_mangle)]
    unsafe extern "C" fn FfiStruct__new(
        input: RefMutSlice<u8>,
        output: *mut *mut ExternFfiStruct,
    ) -> FfiReturn {
        let string = unsafe { input.into_rust() }.expect("Defined").to_vec();
        let string = String::from_utf8(string).expect("Valid UTF8 string");
        let opaque = Box::new(ExternFfiStruct(string));
        unsafe { output.write(Box::into_raw(opaque)) };
        FfiReturn::Ok
    }
}
