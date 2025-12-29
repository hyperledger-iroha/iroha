//! FFI import/getset tests: validates generated getters/setters across FFI.
#![allow(unsafe_code)]

use iroha_ffi::{ffi, ffi_import};

// Localize macro-generated items in a module; re-export for tests.
mod types {
    //! FFI types used for import/getset coverage.
    use super::*;
    ffi! {
        /// Wrapper type imported from FFI.
        #[derive(Clone, PartialEq, Eq)]
        pub struct Name;

        /// Struct imported from FFI for accessor validation.
        #[ffi_import]
        #[derive(Clone, PartialEq, Eq, Setters, Getters, MutGetters)]
        #[getset(get = "pub")]
        #[ffi_type(opaque)]
        #[repr(C)]
        pub struct FfiStruct {
            /// Stored identifier value.
            #[getset(set = "pub", get_mut = "pub")]
            id: u8,
            /// Nested `Name` field.
            name: Name,
        }
    }

    /// Public shim to obtain a `RefName` without calling a private method across the module boundary.
    pub fn name_as_ref(v: &super::Name) -> super::RefName<'_> {
        v.as_ref()
    }
}

pub use types::*;

iroha_ffi::handles! {Name, FfiStruct}
iroha_ffi::decl_ffi_fns! {Drop, Clone, Eq}

#[ffi_import]
impl Name {
    /// Construct a new `Name` from a `String`.
    pub fn new(name: String) -> Self {
        unreachable!("replaced by ffi_import")
    }
}

#[ffi_import]
impl FfiStruct {
    /// Construct a new `FfiStruct` from parts.
    pub fn new(name: String, id: u8) -> Self {
        unreachable!("replaced by ffi_import")
    }
}

#[test]
fn import_shared_fns() {
    let mut ffi_struct = FfiStruct::new("ipso facto".to_string(), 42);
    ffi_struct.set_id(84);
    assert!(&mut 84 == ffi_struct.id_mut());

    assert!(*types::name_as_ref(&Name::new("ipso facto".to_string())) == *ffi_struct.name());
}

mod ffi {
    use iroha_ffi::{
        FfiConvert, FfiOutPtr, FfiOutPtrWrite, FfiReturn, FfiType, def_ffi_fns, slice::RefMutSlice,
    };

    iroha_ffi::handles! {ExternName, ExternFfiStruct}

    def_ffi_fns! { dealloc }
    def_ffi_fns! {
        Drop: {ExternName, ExternFfiStruct},
        Clone: {ExternName, ExternFfiStruct},
        Eq: {ExternName, ExternFfiStruct},
    }

    /// Structure that `Name` points to.
    #[derive(Debug, Clone, PartialEq, Eq, FfiType)]
    #[ffi_type(opaque)]
    #[repr(C)]
    pub struct ExternName(String);

    /// Structure that `FfiStruct` points to.
    #[derive(Debug, Clone, PartialEq, Eq, FfiType)]
    #[ffi_type(opaque)]
    #[repr(C)]
    pub struct ExternFfiStruct {
        id: u8,
        name: ExternName,
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn Name__new(
        input1: RefMutSlice<u8>,
        output: *mut *mut ExternName,
    ) -> FfiReturn {
        let string = unsafe { input1.into_rust() }.expect("Defined").to_vec();
        let string = String::from_utf8(string).expect("Valid UTF8 string");
        let opaque = Box::new(ExternName(string));
        unsafe { output.write(Box::into_raw(opaque)) };
        FfiReturn::Ok
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn FfiStruct__new(
        input1: RefMutSlice<u8>,
        input2: <u8 as FfiType>::ReprC,
        output: *mut *mut ExternFfiStruct,
    ) -> FfiReturn {
        let string = unsafe { input1.into_rust() }.expect("Defined").to_vec();
        let string = String::from_utf8(string).expect("Valid UTF8 string");
        let num = unsafe { FfiConvert::try_from_ffi(input2, &mut ()) }.expect("Valid num");
        let name = ExternName(string);
        let opaque = Box::new(ExternFfiStruct { id: num, name });
        unsafe { output.write(Box::into_raw(opaque)) };
        FfiReturn::Ok
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn FfiStruct__id(
        input: *const ExternFfiStruct,
        output: *mut <&u8 as FfiType>::ReprC,
    ) -> FfiReturn {
        let input = unsafe { &*input };
        unsafe { output.write(&raw const input.id) };
        FfiReturn::Ok
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn FfiStruct__id_mut(
        input: *mut ExternFfiStruct,
        output: *mut <&mut u8 as FfiType>::ReprC,
    ) -> FfiReturn {
        let input = unsafe { &mut *input };
        unsafe { output.write(&raw mut input.id) };
        FfiReturn::Ok
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn FfiStruct__set_id(
        input: *mut ExternFfiStruct,
        id: <u8 as FfiType>::ReprC,
    ) -> FfiReturn {
        let input = unsafe { &mut *input };
        let value = unsafe { FfiConvert::try_from_ffi(id, &mut ()) }.expect("Valid num");
        input.id = value;
        FfiReturn::Ok
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn FfiStruct__name(
        input: *const ExternFfiStruct,
        output: *mut <&ExternName as FfiOutPtr>::OutPtr,
    ) -> FfiReturn {
        let input = unsafe { &*input };
        unsafe { FfiOutPtrWrite::write_out(&input.name, output) };
        FfiReturn::Ok
    }
}
